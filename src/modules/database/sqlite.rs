use super::model::{
    CatalogColumn, CatalogDatabase, CatalogSchema, CatalogTable, CellValue, DatabaseCatalog,
    DatabaseConnection, QueryResult,
};
use anyhow::{Context, Result};
use rusqlite::{types::ValueRef, Connection, InterruptHandle};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Execute one SQLite statement on the caller's background thread.
///
/// `on_ready` receives the interrupt handle before the statement starts so the
/// UI can cancel long-running queries without sharing the connection.
pub fn execute(
    path: &Path,
    sql: &str,
    max_rows: usize,
    on_ready: impl FnOnce(InterruptHandle),
) -> Result<QueryResult> {
    let started = Instant::now();
    let connection = Connection::open(path)
        .with_context(|| format!("无法打开 SQLite 数据库：{}", path.display()))?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .context("无法设置 SQLite 忙等待时间")?;
    on_ready(connection.get_interrupt_handle());

    let mut statement = connection
        .prepare(sql)
        .with_context(|| format!("SQL 无法执行：{sql}"))?;
    let column_count = statement.column_count();
    let mut result = QueryResult {
        sql: sql.to_string(),
        elapsed_ms: 0,
        ..QueryResult::default()
    };
    if column_count == 0 {
        result.affected_rows = Some(statement.execute([])?);
    } else {
        result.columns = (0..column_count)
            .map(|index| statement.column_name(index).unwrap_or("?").to_string())
            .collect();
        let row_limit = max_rows.clamp(1, 10_000);
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if result.rows.len() >= row_limit {
                result.truncated = true;
                break;
            }
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                values.push(match row.get_ref(index)? {
                    ValueRef::Null => CellValue::Null,
                    ValueRef::Integer(value) => CellValue::Integer(value),
                    ValueRef::Real(value) => CellValue::Real(value),
                    ValueRef::Text(value) => {
                        CellValue::Text(String::from_utf8_lossy(value).into_owned())
                    }
                    ValueRef::Blob(value) => CellValue::Blob(value.len()),
                });
            }
            result.rows.push(values);
        }
    }
    result.elapsed_ms = started.elapsed().as_millis();
    Ok(result)
}

pub fn friendly_error(error: &anyhow::Error) -> String {
    let text = format!("{error:#}");
    if text.contains("interrupted") || text.contains("Interrupted") {
        "查询已取消".into()
    } else {
        text
    }
}

pub fn catalog(connection: &DatabaseConnection) -> Result<DatabaseCatalog> {
    let sqlite = Connection::open(&connection.path)
        .with_context(|| format!("无法打开 SQLite 数据库：{}", connection.path.display()))?;
    let mut tables = Vec::new();
    let mut statement = sqlite.prepare(
        "SELECT name, type FROM sqlite_master
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let entries = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for entry in entries {
        let (name, kind) = entry?;
        let mut columns = Vec::new();
        let mut info = sqlite.prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            name.replace('"', "\"\"")
        ))?;
        let rows = info.query_map([], |row| {
            Ok(CatalogColumn {
                name: row.get(1)?,
                data_type: row.get(2)?,
                nullable: row.get::<_, i64>(3)? == 0,
                primary_key: row.get::<_, i64>(5)? > 0,
                default_value: row.get(4)?,
            })
        })?;
        for column in rows {
            columns.push(column?);
        }
        tables.push(CatalogTable {
            name,
            kind,
            columns,
        });
    }
    let database = connection
        .path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(DatabaseCatalog {
        databases: vec![CatalogDatabase {
            name: database,
            schemas: vec![CatalogSchema {
                name: "main".into(),
                tables,
            }],
        }],
    })
}
