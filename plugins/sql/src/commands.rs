// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use indexmap::IndexMap;
use serde_json::Value as JsonValue;
// use sqlx::migrate::Migrator; // Removed
use tauri::{command, AppHandle, Runtime, State};
use tauri::Manager; // Added import for path() method

// Updated imports
// use crate::{DbInstances, DbPool, Error, LastInsertId, Migrations};
use crate::{ConnectionManager, Error, LastInsertId, TransactionManager, convert}; // Removed DbInfo
// use std::path::{Path, PathBuf}; // Remove unused Path
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;
use std::sync::{Arc, Mutex}; // Added missing import

// Refactored load command
#[command]
pub(crate) fn load<R: Runtime>( // Removed async
    app: AppHandle<R>,
    connections: State<'_, ConnectionManager>,
    db: String,
) -> Result<String, crate::Error> {
    // Parse the alias
    let (kind, path_part) = db.split_once(':').ok_or_else(|| Error::InvalidDatabaseUrl(db.clone()))?;
    if kind != "sqlite" {
        return Err(Error::UnsupportedDatabaseType(kind.to_string()));
    }

    let path = if path_part == ":memory:" {
        PathBuf::from(":memory:")
    } else {
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::Io(format!("Failed to get app_data_dir: {}", e)))?;
        let resolved_path = base_dir.join(path_part);
        if let Some(parent_dir) = resolved_path.parent() {
            std::fs::create_dir_all(parent_dir).map_err(|e| Error::Io(format!("Failed to create parent directory: {}", e)))?;
        }
        resolved_path
    };

    // Open connection
    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| Error::ConnectionFailed(path.display().to_string(), e.to_string()))?;

    let conn_arc = Arc::new(Mutex::new(conn));

    // Insert Arc<Mutex<Connection>> into the manager
    let mut connection_map = connections.inner().0.lock().unwrap();
    if connection_map.contains_key(&db) {
        log::warn!("Database alias '{}' already loaded. Overwriting connection.", db);
    }
    connection_map.insert(db.clone(), conn_arc);

    Ok(db)
}

/// Allows the database connection(s) to be closed; if no database
/// name is passed in then _all_ database connection pools will be
/// shut down.
#[command]
pub(crate) fn close( // Removed async as no async ops needed now
    connections: State<'_, ConnectionManager>,
    // transactions: State<'_, TransactionManager>, // TODO: Handle open transactions?
    db: Option<String>,
) -> Result<bool, crate::Error> { // Changed return to match old signature (bool)
    let mut connection_map = connections.inner().0.lock().unwrap();

    let aliases_to_remove = if let Some(db_alias) = db {
        if !connection_map.contains_key(&db_alias) {
            // Return Ok(false) or Error? Old code returned Error::DatabaseNotLoaded.
            // Let's stick to that for now.
            return Err(Error::DatabaseNotLoaded(db_alias));
        }
        vec![db_alias]
    } else {
        connection_map.keys().cloned().collect()
    };

    for alias in aliases_to_remove {
        connection_map.remove(&alias);
        // Connection associated with the Arc<Mutex<_>> will be closed when Arc count drops to 0.
        // TODO: Check TransactionManager and cleanup related transactions?
        // If a transaction holds an Arc clone, the connection won't close until TX finishes.
    }

    Ok(true)
}

// --- Transaction Commands --- Implementation ---

#[command]
pub(crate) fn begin_transaction(
    connections: State<'_, ConnectionManager>,
    transactions: State<'_, TransactionManager>,
    db_alias: String,
) -> Result<String, Error> {
    // Get Arc<Mutex<Connection>> from ConnectionManager
    let conn_arc = connections
        .inner()
        .0
        .lock()
        .unwrap()
        .get(&db_alias)
        // Clone the Arc here to move into the transaction map
        .cloned()
        .ok_or_else(|| Error::DatabaseNotLoaded(db_alias.clone()))?;

    // Lock the connection to begin transaction
    let conn_guard = conn_arc.lock().unwrap();
    conn_guard
        .execute_batch("BEGIN DEFERRED")
        .map_err(Error::Rusqlite)?;
    // Drop guard immediately after BEGIN, allowing other ops outside the TX manager
    drop(conn_guard);

    // Generate ID and store the cloned Arc
    let tx_id = Uuid::new_v4();
    transactions
        .inner()
        .0
        .lock()
        .unwrap()
        .insert(tx_id, conn_arc); // Store the cloned Arc

    Ok(tx_id.to_string())
}

#[command]
pub(crate) fn commit_transaction(
    transactions: State<'_, TransactionManager>,
    tx_id: String,
) -> Result<(), Error> {
    let uuid = Uuid::from_str(&tx_id).map_err(|_| Error::InvalidUuid(tx_id.clone()))?;

    // Ensure correct State access
    let maybe_conn = transactions.inner().0.lock().unwrap().remove(&uuid);

    if let Some(arc_mutex_conn) = maybe_conn {
        let conn_guard = arc_mutex_conn.lock().unwrap();
        conn_guard.execute_batch("COMMIT").map_err(Error::Rusqlite)?;
        Ok(())
    } else {
        Err(Error::TransactionNotFound(tx_id))
    }
}

#[command]
pub(crate) fn rollback_transaction(
    transactions: State<'_, TransactionManager>,
    tx_id: String,
) -> Result<(), Error> {
     let uuid = Uuid::from_str(&tx_id).map_err(|_| Error::InvalidUuid(tx_id.clone()))?;

    // Ensure correct State access
    let maybe_conn = transactions.inner().0.lock().unwrap().remove(&uuid);

    if let Some(arc_mutex_conn) = maybe_conn {
        let conn_guard = arc_mutex_conn.lock().unwrap();
        // Log rollback errors but don't propagate them as the transaction state is cleared anyway
        if let Err(e) = conn_guard.execute_batch("ROLLBACK") {
            log::error!("Error rolling back transaction {}: {}", tx_id, e);
        }
        Ok(())
    } else {
        Err(Error::TransactionNotFound(tx_id))
    }
}

// --- Existing Commands to be Refactored (Step 6 & 7) ---

/// Execute a command against the database
#[command]
pub(crate) fn execute(
    connections: State<'_, ConnectionManager>,
    transactions: State<'_, TransactionManager>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
    tx_id: Option<String>,
) -> Result<(u64, LastInsertId), crate::Error> {
    let converted_params = convert::json_to_rusqlite_params(values)?;

    // Get the connection Arc, either from TransactionManager or ConnectionManager
    let conn_arc = if let Some(tx_id_str) = tx_id {
        let uuid = Uuid::from_str(&tx_id_str).map_err(|_| Error::InvalidUuid(tx_id_str.clone()))?;
        let tx_map = transactions.inner().0.lock().unwrap();
        tx_map
            .get(&uuid)
            .cloned() // Clone the Arc<Mutex<Conn>>
            .ok_or_else(|| Error::TransactionNotFound(tx_id_str))?
    } else {
        let conn_map = connections.inner().0.lock().unwrap();
        conn_map
            .get(&db) // Use db alias here
            .cloned() // Clone the Arc<Mutex<Conn>>
            .ok_or_else(|| Error::DatabaseNotLoaded(db.clone()))?
    };

    // Lock the connection and execute
    let conn_guard = conn_arc.lock().unwrap();
    let changes = conn_guard
        .execute(&query, rusqlite::params_from_iter(converted_params))
        .map_err(Error::Rusqlite)?;
    let last_id = conn_guard.last_insert_rowid();
    Ok((changes as u64, LastInsertId::Sqlite(last_id)))
}

#[command]
pub(crate) fn select(
    connections: State<'_, ConnectionManager>,
    transactions: State<'_, TransactionManager>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
    tx_id: Option<String>,
) -> Result<Vec<IndexMap<String, JsonValue>>, crate::Error> {
    let converted_params = convert::json_to_rusqlite_params(values)?;

    // Get the connection Arc, either from TransactionManager or ConnectionManager
    let conn_arc = if let Some(tx_id_str) = tx_id {
        let uuid = Uuid::from_str(&tx_id_str).map_err(|_| Error::InvalidUuid(tx_id_str.clone()))?;
        let tx_map = transactions.inner().0.lock().unwrap();
        tx_map
            .get(&uuid)
            .cloned()
            .ok_or_else(|| Error::TransactionNotFound(tx_id_str))?
    } else {
        let conn_map = connections.inner().0.lock().unwrap();
        conn_map
            .get(&db)
            .cloned()
            .ok_or_else(|| Error::DatabaseNotLoaded(db.clone()))?
    };

    // Lock the connection and execute select
    let conn_guard = conn_arc.lock().unwrap();
    let mut stmt = conn_guard.prepare(&query).map_err(Error::Rusqlite)?;
    let col_names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let mut rows = stmt.query(rusqlite::params_from_iter(converted_params)).map_err(Error::Rusqlite)?;

    let mut result_vec = Vec::new();
    while let Some(row) = rows.next().map_err(Error::Rusqlite)? {
        let mut row_map = IndexMap::new();
        for (i, col_name) in col_names.iter().enumerate() {
            let value_ref = row.get_ref(i).map_err(Error::Rusqlite)?;
            let value_json = convert::rusqlite_value_to_json(value_ref)?;
            row_map.insert(col_name.clone(), value_json);
        }
        result_vec.push(row_map);
    }
    Ok(result_vec)
}
