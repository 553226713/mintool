use super::{
    driver::{quote_sql, QueryCancel},
    model::{
        CatalogColumn, CatalogDatabase, CatalogSchema, CatalogTable, CellValue, DatabaseCatalog,
        DatabaseConnection, QueryResult,
    },
};
use anyhow::{Context, Result};
use mysql::{prelude::Queryable, Conn, OptsBuilder, Value};
use std::time::Instant;

fn connect(connection: &DatabaseConnection, database: Option<&str>) -> Result<Conn> {
    let mut builder = OptsBuilder::new()
        .ip_or_hostname(Some(connection.host.clone()))
        .tcp_port(connection.port)
        .user(Some(connection.user.clone()));
    if let Some(password) = &connection.password {
        builder = builder.pass(Some(password.clone()));
    }
    if let Some(database) = database.filter(|database| !database.is_empty()) {
        builder = builder.db_name(Some(database.to_string()));
    }
    Conn::new(builder).context("MySQL/MariaDB 连接失败")
}

pub fn execute(
    connection: &DatabaseConnection,
    sql: &str,
    max_rows: usize,
    on_ready: impl FnOnce(QueryCancel),
) -> Result<QueryResult> {
    let database = connection.database.trim();
    let mut client = connect(connection, (!database.is_empty()).then_some(database))?;
    on_ready(QueryCancel::Unsupported);
    let started = Instant::now();
    let mut query_result = client
        .query_iter(sql)
        .with_context(|| format!("SQL 无法执行：{sql}"))?;
    let row_limit = max_rows.clamp(1, 10_000);
    let mut result = QueryResult {
        sql: sql.into(),
        ..QueryResult::default()
    };
    while let Some(mut set) = query_result.iter() {
        result.columns = set
            .columns()
            .as_ref()
            .iter()
            .map(|column| column.name_str().into_owned())
            .collect();
        result.rows.clear();
        result.affected_rows = Some(set.affected_rows() as usize);
        result.truncated = false;
        for row in &mut set {
            let row = row?;
            if result.rows.len() >= row_limit {
                result.truncated = true;
                break;
            }
            result.rows.push(
                (0..row.len())
                    .map(|index| mysql_cell(row.get::<Value, _>(index)))
                    .collect(),
            );
        }
    }
    result.elapsed_ms = started.elapsed().as_millis();
    Ok(result)
}

fn mysql_cell(value: Option<Value>) -> CellValue {
    match value {
        None | Some(Value::NULL) => CellValue::Null,
        Some(Value::Bytes(bytes)) => CellValue::Text(String::from_utf8_lossy(&bytes).into_owned()),
        Some(Value::Int(value)) => CellValue::Integer(value),
        Some(Value::UInt(value)) => CellValue::Text(value.to_string()),
        Some(Value::Float(value)) => CellValue::Real(value as f64),
        Some(Value::Double(value)) => CellValue::Real(value),
        Some(Value::Date(year, month, day, hour, minute, second, micros)) => {
            CellValue::Text(if hour == 0 && minute == 0 && second == 0 && micros == 0 {
                format!("{year:04}-{month:02}-{day:02}")
            } else if micros == 0 {
                format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
            } else {
                format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{micros:06}"
                )
            })
        }
        Some(Value::Time(negative, days, hours, minutes, seconds, micros)) => {
            let sign = if negative { "-" } else { "" };
            CellValue::Text(format!(
                "{sign}{:03}:{hours:02}:{minutes:02}:{seconds:02}.{micros:06}",
                days * 24
            ))
        }
    }
}

pub fn catalog(connection: &DatabaseConnection) -> Result<DatabaseCatalog> {
    let mut client = connect(connection, None)?;
    let databases = query_rows(&mut client, "SHOW DATABASES")?
        .into_iter()
        .filter_map(|row| row.first().cloned().flatten())
        .map(|value| value_to_string(&value))
        .filter(|database| {
            !matches!(
                database.as_str(),
                "information_schema" | "mysql" | "performance_schema" | "sys"
            )
        })
        .collect::<Vec<_>>();
    let mut catalog_databases = Vec::new();
    for database in databases {
        let tables = query_rows(
            &mut client,
            &format!(
                "SELECT TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES
                 WHERE TABLE_SCHEMA = {} ORDER BY TABLE_NAME",
                quote_sql(&database)
            ),
        )?;
        let mut catalog_tables = Vec::new();
        for table in tables {
            let Some(table_value) = table.first().cloned().flatten() else {
                continue;
            };
            let table_name = value_to_string(&table_value);
            let kind = table
                .get(1)
                .cloned()
                .flatten()
                .map_or_else(String::new, |value| value_to_string(&value));
            let columns = query_rows(
                &mut client,
                &format!(
                    "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT,
                            COLUMN_KEY = 'PRI'
                     FROM information_schema.COLUMNS
                     WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {}
                     ORDER BY ORDINAL_POSITION",
                    quote_sql(&database),
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
                            name: value_to_string(row.first()?.as_ref()?),
                            data_type: value_to_string(row.get(1)?.as_ref()?),
                            nullable: value_to_string(row.get(2)?.as_ref()?)
                                .eq_ignore_ascii_case("YES"),
                            primary_key: value_to_string(row.get(4)?.as_ref()?) == "1",
                            default_value: row
                                .get(3)
                                .cloned()
                                .flatten()
                                .map(|value| value_to_string(&value)),
                        })
                    })
                    .collect(),
            });
        }
        catalog_databases.push(CatalogDatabase {
            name: database.clone(),
            schemas: vec![CatalogSchema {
                name: database,
                tables: catalog_tables,
            }],
        });
    }
    Ok(DatabaseCatalog {
        databases: catalog_databases,
    })
}

fn query_rows(client: &mut Conn, sql: &str) -> Result<Vec<Vec<Option<Value>>>> {
    let rows = client.query::<mysql::Row, _>(sql)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (0..row.len())
                .map(|index| row.get::<Value, _>(index))
                .collect()
        })
        .collect())
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::NULL => "NULL".into(),
        Value::Bytes(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        Value::Int(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Double(value) => value.to_string(),
        Value::Date(..) | Value::Time(..) => mysql_cell(Some(value.clone())).display(),
    }
}
