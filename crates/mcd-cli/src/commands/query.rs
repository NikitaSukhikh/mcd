use std::path::Path;

use anyhow::{Context, Result, bail};
use mcd_core::{
    McdPackage,
    schema::ColumnType,
    tables::{TypedValue, load_manifest_tables},
};
use rusqlite::{
    Connection, params_from_iter,
    types::{Value, ValueRef},
};
use serde_json::{Map, Value as JsonValue, json};

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

pub fn run(file: &Path, sql: &str, format: OutputFormat) -> Result<()> {
    validate_read_only_sql(sql)?;

    let package = McdPackage::open_path(file)?;
    let manifest = package.manifest()?;
    let tables = load_manifest_tables(&package, &manifest)?;
    let mut connection = Connection::open_in_memory()?;
    connection.execute_batch("PRAGMA query_only = OFF;")?;
    load_tables_into_sqlite(&mut connection, tables.values())?;
    connection.execute_batch("PRAGMA query_only = ON;")?;

    let mut statement = connection
        .prepare(sql)
        .with_context(|| "prepare SQL query")?;
    if !statement.readonly() {
        bail!("query must be read-only");
    }

    let result = query_rows(&mut statement)?;
    match format {
        OutputFormat::Table => print_table(&result),
        OutputFormat::Json => print_json(&result)?,
        OutputFormat::Csv => print_csv(&result),
    }
    Ok(())
}

fn validate_read_only_sql(sql: &str) -> Result<()> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        bail!("SQL query cannot be empty");
    }
    let without_final_semicolon = trimmed.trim_end_matches(';').trim_end();
    if without_final_semicolon.contains(';') {
        bail!("query must contain exactly one SQL statement");
    }

    let lowercase = trimmed.to_ascii_lowercase();
    if !(lowercase.starts_with("select") || lowercase.starts_with("with")) {
        bail!("query must be a SELECT statement");
    }
    Ok(())
}

fn load_tables_into_sqlite<'a>(
    connection: &mut Connection,
    tables: impl IntoIterator<Item = &'a mcd_core::tables::DataTable>,
) -> Result<()> {
    let transaction = connection.transaction()?;
    for table in tables {
        let column_defs = table
            .schema
            .columns
            .iter()
            .map(|column| {
                format!(
                    "{} {}",
                    quote_identifier(&column.name),
                    sqlite_type(column.value_type)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        transaction.execute(
            &format!(
                "CREATE TABLE {} ({column_defs})",
                quote_identifier(&table.id)
            ),
            [],
        )?;

        let placeholders = std::iter::repeat_n("?", table.schema.columns.len())
            .collect::<Vec<_>>()
            .join(", ");
        let insert_sql = format!(
            "INSERT INTO {} VALUES ({placeholders})",
            quote_identifier(&table.id)
        );
        let mut insert = transaction.prepare(&insert_sql)?;
        for row in &table.rows {
            let values = table
                .schema
                .columns
                .iter()
                .map(|column| {
                    let value = row
                        .cells
                        .get(&column.name)
                        .with_context(|| format!("missing cell '{}'", column.name))?;
                    Ok(sqlite_value(value))
                })
                .collect::<Result<Vec<_>>>()?;
            insert.execute(params_from_iter(values))?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn sqlite_type(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::Integer | ColumnType::Boolean => "INTEGER",
        ColumnType::Decimal => "REAL",
        ColumnType::String
        | ColumnType::Date
        | ColumnType::Datetime
        | ColumnType::Time
        | ColumnType::Enum => "TEXT",
    }
}

fn sqlite_value(value: &TypedValue) -> Value {
    match value {
        TypedValue::Null => Value::Null,
        TypedValue::String(value)
        | TypedValue::Decimal(value)
        | TypedValue::Date(value)
        | TypedValue::Datetime(value)
        | TypedValue::Time(value)
        | TypedValue::Enum(value) => Value::Text(value.clone()),
        TypedValue::Integer(value) => Value::Integer(*value),
        TypedValue::Boolean(value) => Value::Integer(i64::from(*value)),
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<QueryValue>>,
}

#[derive(Clone)]
enum QueryValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

fn query_rows(statement: &mut rusqlite::Statement<'_>) -> Result<QueryResult> {
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let column_count = columns.len();
    let mut rows = Vec::new();
    let mut query = statement.query([])?;
    while let Some(row) = query.next()? {
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            values.push(query_value(row.get_ref(index)?));
        }
        rows.push(values);
    }
    Ok(QueryResult { columns, rows })
}

fn query_value(value: ValueRef<'_>) -> QueryValue {
    match value {
        ValueRef::Null => QueryValue::Null,
        ValueRef::Integer(value) => QueryValue::Integer(value),
        ValueRef::Real(value) => QueryValue::Real(value),
        ValueRef::Text(value) => QueryValue::Text(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => QueryValue::Blob(value.to_vec()),
    }
}

fn print_table(result: &QueryResult) {
    let mut widths = result.columns.iter().map(String::len).collect::<Vec<_>>();
    for row in &result.rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_value(value).len());
        }
    }

    print_separator(&widths);
    print_row(&result.columns, &widths);
    print_separator(&widths);
    for row in &result.rows {
        let cells = row.iter().map(display_value).collect::<Vec<_>>();
        print_row(&cells, &widths);
    }
    print_separator(&widths);
    println!("{} row(s)", result.rows.len());
}

fn print_separator(widths: &[usize]) {
    let parts = widths
        .iter()
        .map(|width| "-".repeat(width + 2))
        .collect::<Vec<_>>();
    println!("+{}+", parts.join("+"));
}

fn print_row(cells: &[String], widths: &[usize]) {
    let cells = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| format!(" {:width$} ", cell, width = widths[index]))
        .collect::<Vec<_>>();
    println!("|{}|", cells.join("|"));
}

fn print_json(result: &QueryResult) -> Result<()> {
    let rows = result
        .rows
        .iter()
        .map(|row| {
            let mut object = Map::new();
            for (index, value) in row.iter().enumerate() {
                object.insert(result.columns[index].clone(), json_value(value));
            }
            JsonValue::Object(object)
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "rows": rows }))?
    );
    Ok(())
}

fn print_csv(result: &QueryResult) {
    println!(
        "{}",
        result
            .columns
            .iter()
            .map(|cell| csv_escape(cell))
            .collect::<Vec<_>>()
            .join(",")
    );
    for row in &result.rows {
        println!(
            "{}",
            row.iter()
                .map(|value| csv_escape(&display_value(value)))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
}

fn display_value(value: &QueryValue) -> String {
    match value {
        QueryValue::Null => String::new(),
        QueryValue::Integer(value) => value.to_string(),
        QueryValue::Real(value) => value.to_string(),
        QueryValue::Text(value) => value.clone(),
        QueryValue::Blob(value) => format!("<{} bytes>", value.len()),
    }
}

fn json_value(value: &QueryValue) -> JsonValue {
    match value {
        QueryValue::Null => JsonValue::Null,
        QueryValue::Integer(value) => json!(value),
        QueryValue::Real(value) => json!(value),
        QueryValue::Text(value) => json!(value),
        QueryValue::Blob(value) => json!(value),
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}
