use super::{
    model::{DatabaseCatalog, DatabaseConnection, DatabaseEngine, QueryResult},
    mysql_driver, postgres_driver, sqlite,
};
use anyhow::Result;

pub enum QueryCancel {
    Sqlite(rusqlite::InterruptHandle),
    Unsupported,
}

impl QueryCancel {
    pub fn interrupt(self) {
        if let Self::Sqlite(handle) = self {
            handle.interrupt();
        }
    }

    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Sqlite(_))
    }
}

pub fn execute(
    connection: &DatabaseConnection,
    sql: &str,
    max_rows: usize,
    on_ready: impl FnOnce(QueryCancel),
) -> Result<QueryResult> {
    match connection.engine {
        DatabaseEngine::Sqlite => sqlite::execute(&connection.path, sql, max_rows, |interrupt| {
            on_ready(QueryCancel::Sqlite(interrupt))
        }),
        DatabaseEngine::Postgres => postgres_driver::execute(connection, sql, max_rows, on_ready),
        DatabaseEngine::Mysql | DatabaseEngine::Mariadb => {
            mysql_driver::execute(connection, sql, max_rows, on_ready)
        }
    }
}

pub fn catalog(connection: &DatabaseConnection) -> Result<DatabaseCatalog> {
    match connection.engine {
        DatabaseEngine::Sqlite => sqlite::catalog(connection),
        DatabaseEngine::Postgres => postgres_driver::catalog(connection),
        DatabaseEngine::Mysql | DatabaseEngine::Mariadb => mysql_driver::catalog(connection),
    }
}

pub fn test(connection: &DatabaseConnection) -> Result<()> {
    let sql = match connection.engine {
        DatabaseEngine::Sqlite => "SELECT 1",
        DatabaseEngine::Postgres => "SELECT 1",
        DatabaseEngine::Mysql | DatabaseEngine::Mariadb => "SELECT 1",
    };
    execute(connection, sql, 1, |_| {}).map(|_| ())
}

pub fn friendly_error(error: &anyhow::Error) -> String {
    sqlite::friendly_error(error)
}

pub fn quote_sql(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
