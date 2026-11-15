use mintool::model::{query::statement_at, session::SessionSurface};
use mintool::modules::database::{
    driver,
    model::{CellValue, DatabaseConnection, DatabaseEngine, DatabaseRegistry},
    sqlite,
};
use mintool::workspace::layout::WorkspaceState;
use std::path::Path;
use std::time::Duration;

fn database_file(directory: &Path) -> std::path::PathBuf {
    let path = directory.join("test.sqlite");
    std::fs::write(&path, []).unwrap();
    path
}

#[test]
fn sqlite_driver_executes_writes_and_reads() {
    let directory = tempfile::tempdir().unwrap();
    let path = database_file(directory.path());

    let created = sqlite::execute(
        &path,
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        100,
        |_| {},
    )
    .unwrap();
    assert_eq!(created.affected_rows, Some(0));

    let inserted = sqlite::execute(
        &path,
        "INSERT INTO users (name, score) VALUES ('Ada', 9.5), ('Linus', 8.0)",
        100,
        |_| {},
    )
    .unwrap();
    assert_eq!(inserted.affected_rows, Some(2));

    let selected = sqlite::execute(
        &path,
        "SELECT id, name, score FROM users ORDER BY id",
        100,
        |_| {},
    )
    .unwrap();
    assert_eq!(selected.columns, ["id", "name", "score"]);
    assert_eq!(
        selected.rows,
        vec![
            vec![
                CellValue::Integer(1),
                CellValue::Text("Ada".into()),
                CellValue::Real(9.5)
            ],
            vec![
                CellValue::Integer(2),
                CellValue::Text("Linus".into()),
                CellValue::Real(8.0)
            ],
        ]
    );
}

#[test]
fn sqlite_result_rows_are_limited() {
    let directory = tempfile::tempdir().unwrap();
    let path = database_file(directory.path());
    let result = sqlite::execute(
        &path,
        "WITH RECURSIVE numbers(value) AS (
            SELECT 1
            UNION ALL
            SELECT value + 1 FROM numbers WHERE value < 10
        )
        SELECT value FROM numbers",
        3,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.rows.len(), 3);
    assert!(result.truncated);
}

#[test]
fn sqlite_query_can_be_interrupted() {
    let directory = tempfile::tempdir().unwrap();
    let path = database_file(directory.path());
    let error = sqlite::execute(
        &path,
        "WITH RECURSIVE numbers(value) AS (
            SELECT 1
            UNION ALL
            SELECT value + 1 FROM numbers
        )
        SELECT count(*) FROM numbers",
        100,
        |interrupt| {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                interrupt.interrupt();
            });
        },
    )
    .unwrap_err();
    assert_eq!(sqlite::friendly_error(&error), "查询已取消");
}

#[test]
fn registry_deduplicates_sqlite_paths() {
    let directory = tempfile::tempdir().unwrap();
    let path = database_file(directory.path());
    let mut registry = DatabaseRegistry::default();
    let first = registry.add_sqlite(path.to_str().unwrap()).unwrap();
    let second = registry.add_sqlite(path.to_str().unwrap()).unwrap();
    assert_eq!(first, second);
    assert_eq!(registry.connections.len(), 1);
}

#[test]
fn registry_creates_new_databases_and_groups_connections() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("new.sqlite");
    let mut registry = DatabaseRegistry::default();
    let group = registry.add_group("开发库").unwrap();
    let id = registry
        .add_sqlite_in_group(path.to_str().unwrap(), Some(group.clone()))
        .unwrap();
    assert!(path.is_file());
    assert_eq!(
        registry.connections[0].group.as_deref(),
        Some(group.as_str())
    );
    registry.move_connection(&id, None, None);
    assert_eq!(registry.connections[0].group, None);
    registry.remove_group(&group);
    assert!(registry.groups.is_empty());
}

#[test]
fn bundled_sqlite_fixture_is_queryable() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mintool_test.sqlite");
    let result = sqlite::execute(
        &path,
        "SELECT name FROM demo_users ORDER BY id",
        100,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.rows[0][0], CellValue::Text("Ada Lovelace".into()));
}

#[test]
fn sqlite_catalog_lists_tables_columns_and_keys() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mintool_test.sqlite");
    let connection = DatabaseConnection {
        name: "fixture".into(),
        engine: DatabaseEngine::Sqlite,
        path,
        ..Default::default()
    };
    let catalog = driver::catalog(&connection).unwrap();
    let table = &catalog.databases[0].schemas[0].tables[0];
    assert_eq!(table.name, "demo_users");
    assert!(table.columns.iter().any(|column| {
        column.name == "id" && column.primary_key && column.data_type == "INTEGER"
    }));
    assert!(table
        .columns
        .iter()
        .any(|column| column.name == "email" && !column.nullable));
}

#[test]
fn common_database_engines_expose_default_ports() {
    assert_eq!(DatabaseEngine::Sqlite.default_port(), 0);
    assert_eq!(DatabaseEngine::Postgres.default_port(), 5432);
    assert_eq!(DatabaseEngine::Mysql.default_port(), 3306);
    assert_eq!(DatabaseEngine::Mariadb.default_port(), 3306);
    assert_eq!(DatabaseEngine::ALL.len(), 4);
}

#[test]
fn network_connection_round_trips_with_group_and_private_permissions() {
    use mintool::services::storage::Storage;
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::new(directory.path().to_path_buf()).unwrap();
    let mut registry = DatabaseRegistry::default();
    let group = registry.add_group("开发环境").unwrap();
    let connection = DatabaseConnection {
        name: "orders".into(),
        engine: DatabaseEngine::Postgres,
        group: Some(group),
        host: "127.0.0.1".into(),
        port: 5432,
        database: "orders".into(),
        user: "postgres".into(),
        password: Some("secret".into()),
        ..Default::default()
    };
    let id = registry.upsert(connection).unwrap();
    registry.save(&storage).unwrap();
    let loaded = DatabaseRegistry::load(&storage).unwrap();
    let connection = loaded
        .connections
        .iter()
        .find(|connection| connection.id == id)
        .unwrap();
    assert_eq!(connection.engine, DatabaseEngine::Postgres);
    assert_eq!(connection.host, "127.0.0.1");
    assert_eq!(connection.database, "orders");
    let mode = std::fs::metadata(storage.registry_path(mintool::modules::ModuleId::Database))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn database_session_uses_query_surface() {
    let connection = mintool::modules::database::model::DatabaseConnection {
        id: "db".into(),
        name: "app.sqlite".into(),
        engine: mintool::modules::database::model::DatabaseEngine::Sqlite,
        path: "/tmp/app.sqlite".into(),
        group: None,
        ..Default::default()
    };
    let session = mintool::model::session::SessionView::database(&connection);
    assert_eq!(session.surface, SessionSurface::Query);
    assert_eq!(session.key.id, "db");
    assert_eq!(session.title, "app.sqlite");
}

#[test]
fn current_statement_uses_cursor_and_sql_quotes() {
    let sql = "SELECT 1;\nSELECT ';' AS value;\nSELECT 3;";
    assert_eq!(statement_at(sql, 3), "SELECT 1");
    assert_eq!(statement_at(sql, 15), "SELECT ';' AS value");
    assert_eq!(statement_at(sql, sql.len()), "SELECT 3");
}

#[test]
fn query_workspace_round_trips_without_duplicate_panes() {
    let mut state = WorkspaceState::query();
    state.queries.get_mut(&1).unwrap().sql = "SELECT 42;".into();
    let encoded = serde_json::to_string(&state).unwrap();
    let decoded: WorkspaceState = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.queries[&1].sql, "SELECT 42;");
    assert_eq!(decoded.pane_ids(), vec![1]);
}

#[test]
fn table_query_tabs_resolve_as_documents_and_do_not_persist_auto_run() {
    let mut state = WorkspaceState::query();
    state.add_query(2);
    let query = state.queries.get_mut(&2).unwrap();
    query.title = "demo_users".into();
    query.sql = "SELECT * FROM \"demo_users\" LIMIT 100;".into();
    query.auto_run = true;
    let pane = state.file_pane(2).unwrap();
    assert!(pane.showing_file);
    assert!(!pane.terminal);
    let decoded: WorkspaceState =
        serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert_eq!(decoded.queries[&2].title, "demo_users");
    assert!(!decoded.queries[&2].auto_run);
}
