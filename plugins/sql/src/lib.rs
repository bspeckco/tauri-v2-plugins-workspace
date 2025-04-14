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

// Removed DbInfo struct
// #[derive(Clone, Debug)]
// struct DbInfo {
//     path: PathBuf,
//     // Add flags like read_only later if needed
// }

#[derive(Default, Clone)]
// Changed ConnectionManager to hold Arc<Mutex<Connection>>
pub(crate) struct ConnectionManager(
    pub Arc<Mutex<HashMap<String, Arc<Mutex<rusqlite::Connection>>>>>
);

#[derive(Default, Clone)]
pub(crate) struct TransactionManager(pub Arc<Mutex<HashMap<Uuid, Arc<Mutex<rusqlite::Connection>>>>>); // Make field 0 pub(crate)


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
    use crate::{commands, ConnectionManager, TransactionManager, Builder as SqlBuilder, LastInsertId};
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

        // Load DB (direct call)
        commands::load(
            app_handle.clone(), 
            app_handle.state::<ConnectionManager>(), 
            db_alias.clone()
        ).expect("Failed to load test DB");

        // Create table (direct call)
        let create_table_sql = "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)".to_string();
        let create_result = commands::execute(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            create_table_sql,
            vec![],
            None,
        );
        assert!(create_result.is_ok(), "Create table failed: {:?}", create_result.err());

        // Insert data (direct call)
        let insert_sql = "INSERT INTO users (name) VALUES (?)".to_string();
        let insert_params = vec![JsonValue::String("Alice".to_string())];
        let insert_result = commands::execute(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            insert_sql,
            insert_params,
            None,
        );
        assert!(insert_result.is_ok(), "Insert failed: {:?}", insert_result.err());
        let (rows_affected, last_insert_id) = insert_result.unwrap();
        assert_eq!(rows_affected, 1);
        match last_insert_id {
            LastInsertId::Sqlite(id) => assert_eq!(id, 1),
            #[allow(unreachable_patterns)]
            _ => panic!("Unexpected LastInsertId variant"),
        }

        // Select data (direct call)
        let select_sql = "SELECT id, name FROM users WHERE name = ?".to_string();
        let select_params = vec![JsonValue::String("Alice".to_string())];
        let select_result = commands::select(
            app_handle.state::<ConnectionManager>(),
            app_handle.state::<TransactionManager>(),
            db_alias.clone(),
            select_sql,
            select_params,
            None,
        );
        assert!(select_result.is_ok(), "Select failed: {:?}", select_result.err());

        let selected_data = select_result.unwrap();
        assert_eq!(selected_data.len(), 1);
        let user_row = &selected_data[0];

        let mut expected_row = indexmap::IndexMap::new();
        expected_row.insert("id".to_string(), json!(1));
        expected_row.insert("name".to_string(), json!("Alice"));

        assert_eq!(user_row, &expected_row);
    }

    // ...
}
