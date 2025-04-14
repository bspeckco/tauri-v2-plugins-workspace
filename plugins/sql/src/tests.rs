#[cfg(test)]
mod tests {
    use crate::{commands, ConnectionManager, DbInfo, Error, LastInsertId, TransactionManager, Builder as SqlBuilder};
    use indexmap::IndexMap;
    use serde::Deserialize;
    use serde_json::{json, Value as JsonValue};
    use std::path::PathBuf;
    use tauri::test::{mock_builder, mock_context, MockRuntime, noop_assets};
    use tauri::{AppHandle, Manager, State};
    use tempfile::tempdir;

    fn setup_test_environment() -> (AppHandle<MockRuntime>, ConnectionManager, TransactionManager) {
        let assets = noop_assets();
        let context = mock_context(assets);
        let app = mock_builder()
            .plugin(SqlBuilder::new().build())
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
        let (app_handle, connection_manager, _transaction_manager) = setup_test_environment();
        let db_alias = "sqlite::memory:".to_string();

        let result = commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        );

        assert!(result.is_ok(), "load failed: {:?}", result.err());
        let returned_alias = result.unwrap();
        assert_eq!(returned_alias, db_alias);

        let manager_map = connection_manager.0.lock().unwrap();
        assert!(manager_map.contains_key(&returned_alias));
    }

    #[test]
    fn test_load_file_db() {
        let (app_handle, connection_manager, _transaction_manager) = setup_test_environment();
        let temp_dir = tempdir().expect("Failed to create temp dir for test");
        let db_relative_path = "test_db.sqlite";
        let db_alias = format!("sqlite:{}", db_relative_path);

        let result = commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone(),
        );

        assert!(result.is_ok(), "load failed: {:?}", result.err());
        let returned_alias = result.unwrap();
        assert_eq!(returned_alias, db_alias);

        let manager_map = connection_manager.0.lock().unwrap();
        assert!(manager_map.contains_key(&returned_alias));

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

        commands::load(
            app_handle.clone(),
            app_handle.state::<ConnectionManager>(),
            db_alias.clone()
        ).expect("Failed to load test DB");

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
} 