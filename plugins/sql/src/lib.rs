// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Interface with SQLite databases using rusqlite.

#![doc(
    html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png",
    html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png"
)]

mod commands;
mod convert; // Added module
// mod decode; // Removed
mod error;
// mod wrapper; // Removed

use std::collections::HashMap;
use std::path::PathBuf; // Added import
use std::sync::{Arc, Mutex};
use uuid::Uuid; // Added

pub use error::Error;
// pub use wrapper::DbPool; // Removed

// Removed unused imports for sqlx, futures_core, tokio, migrations
// use futures_core::future::BoxFuture;
// use serde::{Deserialize, Serialize}; // Kept Serialize for LastInsertId, Deserialize for PluginConfig
use serde::Serialize; // Adjusted imports
// use sqlx::{
//     error::BoxDynError,
//     migrate::{Migration as SqlxMigration, MigrationSource, MigrationType, Migrator},
// };
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Manager, Runtime, // Removed RunEvent, AppHandle, State
};
// use tokio::sync::{Mutex as TokioMutex, RwLock}; // Removed tokio::sync

// Removed DbInstances struct
// #[derive(Default)]
// pub struct DbInstances(pub RwLock<HashMap<String, DbPool>>);

// Kept LastInsertId for now, might need adjustment later if return type changes
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum LastInsertId {
    #[cfg(feature = "sqlite")]
    Sqlite(i64),
    // Removed mysql/postgres variants
    // #[cfg(feature = "mysql")]
    // MySql(u64),
    // #[cfg(feature = "postgres")]
    // Postgres(()),
    #[cfg(not(feature = "sqlite"))]
    None,
}


// Removed Migrations struct and related types (MigrationKind, Migration, MigrationList, MigrationSource impl)
// struct Migrations(Mutex<HashMap<String, MigrationList>>);
// ... MigrationKind ...
// ... Migration ...
// ... MigrationList ...
// ... impl MigrationSource ...

// Removed PluginConfig for now as preload is removed
// #[derive(Default, Clone, Deserialize)]
// pub struct PluginConfig {
//     #[serde(default)]
//     preload: Vec<String>,
// }

// Removed run_async_command helper function
// fn run_async_command<F: std::future::Future>(cmd: F) -> F::Output { ... }

// --- New State Definitions ---

// Reintroduce DbInfo
#[derive(Clone, Debug)] // Removed Send + Sync from derive
struct DbInfo {
    path: PathBuf,
}

#[derive(Default, Clone)]
// Revert ConnectionManager to hold DbInfo
pub(crate) struct ConnectionManager(
    pub Arc<Mutex<HashMap<String, DbInfo>>>
);

#[derive(Default, Clone)]
pub(crate) struct TransactionManager(pub Arc<Mutex<HashMap<Uuid, Arc<Mutex<rusqlite::Connection>>>>>);


// --- Updated Builder ---

/// Tauri SQL plugin builder.
#[derive(Default)]
pub struct Builder {
    // Removed migrations field
    // migrations: Option<HashMap<String, MigrationList>>,
}

impl Builder {
    pub fn new() -> Self {
        // Simplified new(), removed sqlx driver check
        Self::default()
    }

    // Removed add_migrations method
    // pub fn add_migrations(...) -> Self { ... }

    // Simplified build method
    // Pass R directly, remove Option<PluginConfig> generic
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::<R>::new("sql") // Removed config generic
            .invoke_handler(tauri::generate_handler![
                commands::load,
                commands::execute,
                commands::select,
                commands::close,
                // Added new transaction commands
                commands::begin_transaction,
                commands::commit_transaction,
                commands::rollback_transaction
            ])
            .setup(|app, _api| { // Removed unused api variable, ensure _api if used
                // Removed old setup logic (preload, migrations)
                // let config = api.config().clone().unwrap_or_default();
                // run_async_command(async move { ... })

                // Register new states
                app.manage(ConnectionManager::default());
                app.manage(TransactionManager::default());

                // Potentially load preconfigured databases here if needed in the future

                Ok(())
            })
            // Removed on_event handler for closing connections
            // .on_event(|app, event| { ... })
            .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::{commands, ConnectionManager, TransactionManager, Builder as SqlBuilder, LastInsertId, Error};
    use serde_json::{json, Value as JsonValue};
    use tauri::{
        test::{mock_builder, mock_context, MockRuntime, noop_assets},
        AppHandle, Manager
    };
    use tempfile::tempdir;

    // Updated test setup helper
    fn setup_test_environment() -> (AppHandle<MockRuntime>, ConnectionManager, TransactionManager) {
        let assets = noop_assets();
        let context = mock_context(assets);
        let app = mock_builder()
            .plugin(SqlBuilder::new().build()) // Keep plugin registered
            .build(context)
            .expect("Failed to build mock app");
        let handle = app.handle().clone();
        let connection_manager = ConnectionManager::default();
        let transaction_manager = TransactionManager::default();
        app.manage(connection_manager.clone());
        app.manage(transaction_manager.clone());
        (handle, connection_manager, transaction_manager)
    }

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_load_in_memory() {
        let (app_handle, _original_connection_manager, _transaction_manager) = setup_test_environment();
        let db_alias = "sqlite::memory:".to_string();

        // Call command directly, getting State from AppHandle
        let result = commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        );

        assert!(result.is_ok(), "load failed: {:?}", result.err());
        let returned_alias = result.unwrap();
        assert_eq!(returned_alias, db_alias);

        // Re-fetch state from app_handle to check modifications
        let final_state = app_handle.state::<ConnectionManager>();
        let manager_map = final_state.inner().0.lock().unwrap();
        assert!(manager_map.contains_key(&returned_alias));
    }

    #[test]
    fn test_load_file_db() {
        let (app_handle, _original_connection_manager, _transaction_manager) = setup_test_environment();
        let _temp_dir = tempdir().expect("Failed to create temp dir for test");
        let db_relative_path = "test_db.sqlite";
        let db_alias = format!("sqlite:{}", db_relative_path);

        // Call command directly, getting State from AppHandle
        let result = commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        );

        assert!(result.is_ok(), "load failed: {:?}", result.err());
        let returned_alias = result.unwrap();
        assert_eq!(returned_alias, db_alias);

        // Re-fetch state from app_handle to check modifications
        let final_state = app_handle.state::<ConnectionManager>();
        let manager_map = final_state.inner().0.lock().unwrap();
        assert!(manager_map.contains_key(&returned_alias));

        // Check file creation
        let test_app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("Failed to get app data dir for test");
        let resolved_expected_path = test_app_data_dir.join(db_relative_path);
        assert!(resolved_expected_path.exists());
        assert!(resolved_expected_path.is_file());
    }

    #[test]
    fn test_basic_execute_select() {
        let (app_handle, _connection_manager, _transaction_manager) = setup_test_environment();
        let db_alias = "sqlite::memory:".to_string();

        // Load DB
        commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        ).expect("Failed to load test DB");

        // --- Perform all operations within a single transaction for consistency ---
        let tx_id = commands::begin_transaction(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone()
        ).expect("Begin transaction failed for test setup");
        let tx_id_opt = Some(tx_id.clone());

        // Create table within TX
        let create_table_sql = "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)".to_string();
        let create_result = commands::execute(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            create_table_sql,
            vec![],
            tx_id_opt.clone(), // Use TX ID
        );
        assert!(create_result.is_ok(), "Create table failed: {:?}", create_result.err());

        // Insert data within TX
        let insert_sql = "INSERT INTO users (name) VALUES (?)".to_string();
        let insert_params = vec![JsonValue::String("Alice".to_string())];
        let insert_result = commands::execute(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            insert_sql,
            insert_params,
            tx_id_opt.clone(), // Use TX ID
        );
        assert!(insert_result.is_ok(), "Insert failed: {:?}", insert_result.err());
        let (rows_affected, last_insert_id) = insert_result.unwrap();
        assert_eq!(rows_affected, 1);
        match last_insert_id {
            LastInsertId::Sqlite(id) => assert_eq!(id, 1),
            #[allow(unreachable_patterns)]
            _ => panic!("Unexpected LastInsertId variant"),
        }

        // Select data within TX
        let select_sql = "SELECT id, name FROM users WHERE name = ?".to_string();
        let select_params = vec![JsonValue::String("Alice".to_string())];
        let select_result = commands::select(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            select_sql,
            select_params,
            tx_id_opt.clone(), // Use TX ID
        );
        assert!(select_result.is_ok(), "Select failed: {:?}", select_result.err());

        let selected_data = select_result.unwrap();
        assert_eq!(selected_data.len(), 1);
        let user_row = &selected_data[0];

        let mut expected_row = indexmap::IndexMap::new();
        expected_row.insert("id".to_string(), json!(1));
        expected_row.insert("name".to_string(), json!("Alice"));

        assert_eq!(user_row, &expected_row);

        // Commit the transaction (clean up)
        commands::commit_transaction(
            app_handle.state::<TransactionManager>(),
            tx_id
        ).expect("Commit failed for test cleanup");
    }

    #[test]
    fn test_transaction_commit() {
        let (app_handle, _connection_manager, _) = setup_test_environment();
        let temp_db_dir = tempdir().expect("Failed to create temp dir for commit test");
        let db_path = temp_db_dir.path().join("test_tx_commit.sqlite");
        let db_alias = format!("sqlite:{}", db_path.display());

        // Load DB
        commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        ).expect("Failed to load test DB");

        // --- Create table in a separate, committed transaction ---
        {
            let setup_tx_id = commands::begin_transaction(
                app_handle.state::<ConnectionManager>(),
                app_handle.state::<TransactionManager>(),
                db_alias.clone()
            ).expect("Begin setup transaction failed");
            let create_table_sql = "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)".to_string();
            commands::execute(
                app_handle.state::<ConnectionManager>(),
                app_handle.state::<TransactionManager>(),
                db_alias.clone(), create_table_sql, vec![], Some(setup_tx_id.clone())
            ).expect("Create table failed in setup transaction");
            commands::commit_transaction(
                app_handle.state::<TransactionManager>(),
                setup_tx_id
            ).expect("Commit setup transaction failed");
        }
        // --- Table creation complete ---

        // Begin main test transaction
        let tx_id = commands::begin_transaction(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone()
        ).expect("Begin transaction failed");
        let tx_id_opt = Some(tx_id.clone());
        let tx_uuid = uuid::Uuid::parse_str(&tx_id).unwrap();

        // Insert data within transaction
        let insert_sql = "INSERT INTO items (id, name) VALUES (?, ?)".to_string();
        let insert_params = vec![json!(1), json!("Item 1")];
        let insert_result = commands::execute(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(), insert_sql, insert_params, tx_id_opt.clone()
        );
        assert!(insert_result.is_ok(), "Insert within TX failed: {:?}", insert_result.err());

        // Select outside transaction (should not see item yet)
        let select_sql = "SELECT name FROM items WHERE id = ?".to_string();
        let select_params = vec![json!(1)];
        let select_outside_result = commands::select(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(), select_sql.clone(), select_params.clone(), None // No tx_id
        );
        assert!(select_outside_result.is_ok(), "Select outside TX failed: {:?}", select_outside_result.err());
        assert!(select_outside_result.unwrap().is_empty(), "Item should not be visible outside TX before commit");

        // Select inside transaction (should see item)
        let select_inside_result = commands::select(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(), select_sql.clone(), select_params.clone(), tx_id_opt.clone() // With tx_id
        );
        assert!(select_inside_result.is_ok(), "Select inside TX failed: {:?}", select_inside_result.err());
        let data_inside = select_inside_result.unwrap();
        assert_eq!(data_inside.len(), 1, "Item should be visible inside TX");
        assert_eq!(data_inside[0].get("name").unwrap(), &json!("Item 1"));

        // Commit transaction
        let commit_result = commands::commit_transaction(
            app_handle.state::<TransactionManager>(),
            tx_id.clone()
        );
        assert!(commit_result.is_ok(), "Commit failed: {:?}", commit_result.err());

        // Select outside transaction again (should see item now)
        let select_after_commit_result = commands::select(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(), select_sql.clone(), select_params.clone(), None // No tx_id
        );
        assert!(select_after_commit_result.is_ok(), "Select after commit failed: {:?}", select_after_commit_result.err());
        let data_after_commit = select_after_commit_result.unwrap();
        assert_eq!(data_after_commit.len(), 1, "Item should be visible outside TX after commit");
        assert_eq!(data_after_commit[0].get("name").unwrap(), &json!("Item 1"));

        // Verify transaction ID is removed from manager (fetch state from app_handle)
        {
            let current_transaction_manager = app_handle.state::<TransactionManager>();
            let tx_map = current_transaction_manager.0.lock().unwrap();
            assert!(!tx_map.contains_key(&tx_uuid), "Transaction ID should be removed after commit");
        }
    }

    #[test]
    fn test_transaction_rollback() {
        let (app_handle, _connection_manager, _) = setup_test_environment();
        let temp_db_dir = tempdir().expect("Failed to create temp dir for rollback test");
        let db_path = temp_db_dir.path().join("test_tx_rollback.sqlite");
        let db_alias = format!("sqlite:{}", db_path.display());

        // Load DB & Create table
        commands::load(app_handle.clone(), app_handle.state::<ConnectionManager>(), db_alias.clone()).expect("Load failed");
        // --- Create table in a separate, committed transaction ---
        {
            let setup_tx_id = commands::begin_transaction(
                app_handle.state::<ConnectionManager>(),
                app_handle.state::<TransactionManager>(),
                db_alias.clone()
            ).expect("Begin setup transaction failed");
            let create_sql = "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)".to_string();
            commands::execute(app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), db_alias.clone(), create_sql, vec![], Some(setup_tx_id.clone())).expect("Create failed in setup transaction");
            commands::commit_transaction(
                app_handle.state::<TransactionManager>(),
                setup_tx_id
            ).expect("Commit setup transaction failed");
        }
        // --- Table creation complete ---

        // Begin main test transaction
        let tx_id = commands::begin_transaction(app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), db_alias.clone()).expect("Begin failed");
        let tx_id_opt = Some(tx_id.clone());
        let tx_uuid = uuid::Uuid::parse_str(&tx_id).unwrap();

        // Insert data within transaction
        let insert_sql = "INSERT INTO items (id, name) VALUES (?, ?)".to_string();
        let insert_params = vec![json!(1), json!("Item R")];
        let insert_result = commands::execute(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), 
            db_alias.clone(), insert_sql, insert_params, tx_id_opt.clone()
        );
        assert!(insert_result.is_ok());

        // Select inside transaction (should see item)
        let select_sql = "SELECT name FROM items WHERE id = ?".to_string();
        let select_params = vec![json!(1)];
        let select_inside_result = commands::select(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), 
            db_alias.clone(), select_sql.clone(), select_params.clone(), tx_id_opt
        );
        assert!(select_inside_result.is_ok());
        assert_eq!(select_inside_result.unwrap().len(), 1, "Item should be visible inside TX before rollback");

        // Rollback transaction
        let rollback_result = commands::rollback_transaction(
            app_handle.state::<TransactionManager>(),
            tx_id.clone()
        );
        assert!(rollback_result.is_ok(), "Rollback failed: {:?}", rollback_result.err());

        // Select outside transaction (should NOT see item)
        let select_after_rollback_result = commands::select(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), 
            db_alias.clone(), select_sql.clone(), select_params.clone(), None // No tx_id
        );
        assert!(select_after_rollback_result.is_ok());
        assert!(select_after_rollback_result.unwrap().is_empty(), "Item should NOT be visible outside TX after rollback");

        // Verify transaction ID is removed from manager (fetch state from app_handle)
        {
            let current_transaction_manager = app_handle.state::<TransactionManager>();
            let tx_map = current_transaction_manager.0.lock().unwrap();
            assert!(!tx_map.contains_key(&tx_uuid), "Transaction ID should be removed after rollback");
        }
    }

    #[test]
    fn test_transaction_error_rollback() {
        let (app_handle, _connection_manager, _) = setup_test_environment();
        let temp_db_dir = tempdir().expect("Failed to create temp dir for error rollback test");
        let db_path = temp_db_dir.path().join("test_tx_error_rollback.sqlite");
        // Use absolute path for the alias
        let db_alias = format!("sqlite:{}", db_path.display());

        // Load DB & Create table (using app_handle.state)
        commands::load(app_handle.clone(), app_handle.state::<ConnectionManager>(), db_alias.clone()).expect("Load failed");
        {
            let setup_tx_id = commands::begin_transaction(
                app_handle.state::<ConnectionManager>(),
                app_handle.state::<TransactionManager>(),
                db_alias.clone()
            ).expect("Begin setup transaction failed");
            let create_sql = "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string();
            commands::execute(app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), db_alias.clone(), create_sql, vec![], Some(setup_tx_id.clone())).expect("Create failed in setup transaction");
            commands::commit_transaction(
                app_handle.state::<TransactionManager>(),
                setup_tx_id
            ).expect("Commit setup transaction failed");
        }

        // Begin main test transaction (using app_handle.state)
        let tx_id = commands::begin_transaction(app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(), db_alias.clone()).expect("Begin failed");
        let tx_id_opt = Some(tx_id.clone());
        let tx_uuid = uuid::Uuid::parse_str(&tx_id).unwrap();

        // 1. Insert valid data (using app_handle.state)
        let insert_sql = "INSERT INTO items (id, name) VALUES (?, ?)".to_string();
        let insert_params_1 = vec![json!(10), json!("Item E1")];
        let insert_result_1 = commands::execute(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(),
            db_alias.clone(), insert_sql.clone(), insert_params_1, tx_id_opt.clone()
        );
        assert!(insert_result_1.is_ok(), "First insert in TX failed: {:?}", insert_result_1.err());

        // 2. Attempt invalid insert (using app_handle.state)
        let insert_params_2 = vec![json!(10), json!("Item E2")];
        let insert_result_2 = commands::execute(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(),
            db_alias.clone(), insert_sql.clone(), insert_params_2, tx_id_opt.clone()
        );
        assert!(insert_result_2.is_err(), "Second (invalid) insert should fail");
        match insert_result_2.err().unwrap() {
            Error::Rusqlite(e) => match e {
                rusqlite::Error::SqliteFailure(f, _) => assert_eq!(f.code, rusqlite::ErrorCode::ConstraintViolation),
                _ => panic!("Expected SqliteFailure"),
            },
            _ => panic!("Expected Error::Rusqlite"),
        }

        // 3. Verify transaction ID still exists in manager (fetch state from app_handle)
        {
            let current_transaction_manager = app_handle.state::<TransactionManager>();
            let tx_map_before_rollback = current_transaction_manager.0.lock().unwrap();
            assert!(tx_map_before_rollback.contains_key(&tx_uuid), "Transaction ID should still exist after op error");
        } // Release lock immediately

        // 4. Rollback transaction (using app_handle.state)
        let rollback_result = commands::rollback_transaction(
            app_handle.state::<TransactionManager>(),
            tx_id.clone()
        );
        assert!(rollback_result.is_ok(), "Rollback failed: {:?}", rollback_result.err());

        // 5. Select outside transaction (using app_handle.state)
        let select_sql = "SELECT name FROM items WHERE id = ?".to_string();
        let select_params = vec![json!(10)];
        let select_after_rollback_result = commands::select(
            app_handle.state::<ConnectionManager>(), app_handle.state::<TransactionManager>(),
            db_alias.clone(), select_sql.clone(), select_params.clone(), None
        );
        assert!(select_after_rollback_result.is_ok());
        assert!(select_after_rollback_result.unwrap().is_empty(), "Item from first insert should NOT be visible after rollback");

        // 6. Verify transaction ID is removed from manager (fetch state from app_handle)
        {
            let current_transaction_manager = app_handle.state::<TransactionManager>();
            let tx_map_after_rollback = current_transaction_manager.0.lock().unwrap();
            assert!(!tx_map_after_rollback.contains_key(&tx_uuid), "Transaction ID should be removed after rollback");
        }
    }

    // More tests will be added here...
}
