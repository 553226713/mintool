use super::{
    driver::{quote_sql, QueryCancel},
    model::{
        CatalogColumn, CatalogDatabase, CatalogSchema, CatalogTable, CellValue, DatabaseCatalog,
        DatabaseConnection, QueryResult,
    },
};
use anyhow::{Context, Result};
use postgres::{Client, NoTls, SimpleQueryMessage};
use std::time::{Duration, Instant};

fn connect(connection: &DatabaseConnection) -> Result<Client> {
    let mut config = postgres::Config::new();
    config
        .host(&connection.host)
        .port(connection.port)
        .user(&connection.user)
        .dbname(&connection.query_database())
        .connect_timeout(Duration::from_secs(8));
    if let Some(password) = &connection.password {
        config.password(password);
    }
    config.connect(NoTls).context("PostgreSQL 连接失败")
}

pub fn execute(
    connection: &DatabaseConnection,
    sql: &str,
    max_rows: usize,
    on_ready: impl FnOnce(QueryCancel),
) -> Result<QueryResult> {
    let mut client = connect(connection)?;
    on_ready(QueryCancel::Unsupported);
    let started = Instant::now();
    let messages = client
        .simple_query(sql)
        .with_context(|| format!("SQL 无法执行：{sql}"))?;
    let row_limit = max_rows.clamp(1, 10_000);
    let mut result = QueryResult {
        sql: sql.into(),
        ..QueryResult::default()
    };
    for message in messages {
        match message {
            SimpleQueryMessage::RowDescription(columns) => {
                result.columns = columns
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect();
                result.rows.clear();
                result.affected_rows = None;
                result.truncated = false;
            }
            SimpleQueryMessage::Row(row) => {
                if result.rows.len() < row_limit {
                    result.rows.push(
                        (0..row.len())
                            .map(|index| {
                                row.get(index)
                                    .map_or(CellValue::Null, |value| CellValue::Text(value.into()))
                            })
                            .collect(),
                    );
                } else {
                    result.truncated = true;
                }
            }
            SimpleQueryMessage::CommandComplete(count) if result.columns.is_empty() => {
                result.affected_rows = Some(count as usize);
            }
            _ => {}
        }
    }
    result.elapsed_ms = started.elapsed().as_millis();
    Ok(result)
}

pub fn catalog(connection: &DatabaseConnection) -> Result<DatabaseCatalog> {
    let mut client = connect(connection)?;
    let schema_rows = query_rows(
        &mut client,
        "SELECT schema_name FROM information_schema.schemata
         WHERE schema_name NOT LIKE 'pg_%' AND schema_name <> 'information_schema'
         ORDER BY schema_name",
    )?;
    let mut schemas = Vec::new();
    for schema_name in schema_rows
        .into_iter()
        .filter_map(|row| row.into_iter().next().flatten())
    {
        let tables = query_rows(
            &mut client,
            &format!(
                "SELECT table_name, table_type FROM information_schema.tables
                 WHERE table_schema = {} AND table_type IN ('BASE TABLE', 'VIEW')
                 ORDER BY table_name",
                quote_sql(&schema_name)
            ),
        )?;
        let mut catalog_tables = Vec::new();
        for table in tables {
            let Some(table_name) = table.first().and_then(Clone::clone) else {
                continue;
            };
            let kind = table.get(1).and_then(Clone::clone).unwrap_or_default();
            let columns = query_rows(
                &mut client,
                &format!(
                    "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default,
                            EXISTS (
                                SELECT 1
                                FROM information_schema.table_constraints tc
                                JOIN information_schema.key_column_usage kcu
                                  ON tc.constraint_name = kcu.constraint_name
                                 AND tc.table_schema = kcu.table_schema
                                 AND tc.table_name = kcu.table_name
                                WHERE tc.constraint_type = 'PRIMARY KEY'
                                  AND tc.table_schema = c.table_schema
                                  AND tc.table_name = c.table_name
                                  AND kcu.column_name = c.column_name
                            )
                     FROM information_schema.columns c
                     WHERE c.table_schema = {} AND c.table_name = {}
                     ORDER BY c.ordinal_position",
                    quote_sql(&schema_name),
                    quote_sql(&table_name)
                ),
            )?;
            catalog_tables.push(CatalogTable {
                name: table_name,
                kind,
                columns: columns
                    .into_iter()
                    .filter_map(|row| {
                        Some(CatalogColumn {
                            name: row.first()?.clone()?,
                            data_type: row.get(1)?.clone()?,
                            nullable: row.get(2)?.as_deref() == Some("YES"),
                            primary_key: row.get(4)?.as_deref() == Some("t"),
                            default_value: row.get(3).cloned().flatten(),
                        })
                    })
                    .collect(),
            });
        }
        schemas.push(CatalogSchema {
            name: schema_name,
            tables: catalog_tables,
        });
    }
    Ok(DatabaseCatalog {
        databases: vec![CatalogDatabase {
            name: connection.query_database(),
            schemas,
        }],
    })
}

pub(crate) fn query_rows(client: &mut Client, sql: &str) -> Result<Vec<Vec<Option<String>>>> {
    let messages = client.simple_query(sql)?;
    let mut rows = Vec::new();
    for message in messages {
        if let SimpleQueryMessage::Row(row) = message {
            rows.push(
                (0..row.len())
                    .map(|index| row.get(index).map(str::to_string))
                    .collect(),
            );
        }
    }
    Ok(rows)
}
