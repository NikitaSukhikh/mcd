//! Read-only SQL querying for MCD package tables.

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
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue, json};

/// Run a read-only SQL query against manifest-declared package tables.
pub fn query_package(package: &McdPackage, sql: &str) -> Result<QueryResult> {
    validate_read_only_sql(sql)?;

    let manifest = package.manifest()?;
    let tables = load_manifest_tables(package, &manifest)?;
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

    query_rows(&mut statement)
}

/// Open an MCD package from disk and run a read-only SQL query against its tables.
pub fn query_path(path: impl AsRef<Path>, sql: &str) -> Result<QueryResult> {
    let package = McdPackage::open_path(path)?;
    query_package(&package, sql)
}

/// Structured result of an SQL query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Result column names in query order.
    pub columns: Vec<String>,
    /// Result rows in column order.
    pub rows: Vec<Vec<QueryValue>>,
}

impl QueryResult {
    /// Number of returned rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Return result rows as JSON objects keyed by column name.
    #[must_use]
    pub fn rows_as_json(&self) -> JsonValue {
        JsonValue::Array(
            self.rows
                .iter()
                .map(|row| {
                    let mut object = Map::new();
                    for (index, value) in row.iter().enumerate() {
                        object.insert(self.columns[index].clone(), value.as_json());
                    }
                    JsonValue::Object(object)
                })
                .collect(),
        )
    }

    /// Return a JSON object with columns, rows, and row count.
    #[must_use]
    pub fn as_json(&self) -> JsonValue {
        json!({
            "columns": self.columns,
            "rows": self.rows_as_json(),
            "rowCount": self.row_count(),
        })
    }

    /// Serialize the result as pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.as_json())?)
    }

    /// Serialize the result as CSV.
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut lines = vec![
            self.columns
                .iter()
                .map(|cell| csv_escape(cell))
                .collect::<Vec<_>>()
                .join(","),
        ];
        for row in &self.rows {
            lines.push(
                row.iter()
                    .map(|value| csv_escape(&value.display()))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        lines.join("\n") + "\n"
    }

    /// Serialize the result as a simple ASCII table.
    #[must_use]
    pub fn to_table(&self) -> String {
        let mut widths = self.columns.iter().map(String::len).collect::<Vec<_>>();
        for row in &self.rows {
            for (index, value) in row.iter().enumerate() {
                widths[index] = widths[index].max(value.display().len());
            }
        }

        let mut lines = Vec::new();
        push_separator(&mut lines, &widths);
        push_row(&mut lines, &self.columns, &widths);
        push_separator(&mut lines, &widths);
        for row in &self.rows {
            let cells = row.iter().map(QueryValue::display).collect::<Vec<_>>();
            push_row(&mut lines, &cells, &widths);
        }
        push_separator(&mut lines, &widths);
        lines.push(format!("{} row(s)", self.rows.len()));
        lines.join("\n") + "\n"
    }
}

/// One SQL result cell value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum QueryValue {
    /// SQL NULL.
    Null,
    /// Signed integer value.
    Integer(i64),
    /// Floating point value.
    Real(f64),
    /// UTF-8 text value.
    Text(String),
    /// Binary blob value.
    Blob(Vec<u8>),
}

impl QueryValue {
    /// Convert this value to a plain JSON scalar.
    #[must_use]
    pub fn as_json(&self) -> JsonValue {
        match self {
            Self::Null => JsonValue::Null,
            Self::Integer(value) => json!(value),
            Self::Real(value) => json!(value),
            Self::Text(value) => json!(value),
            Self::Blob(value) => json!(value),
        }
    }

    /// Return a display string suitable for table and CSV output.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Integer(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Blob(value) => format!("<{} bytes>", value.len()),
        }
    }
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

fn push_separator(lines: &mut Vec<String>, widths: &[usize]) {
    let parts = widths
        .iter()
        .map(|width| "-".repeat(width + 2))
        .collect::<Vec<_>>();
    lines.push(format!("+{}+", parts.join("+")));
}

fn push_row(lines: &mut Vec<String>, cells: &[String], widths: &[usize]) {
    let cells = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| format!(" {:width$} ", cell, width = widths[index]))
        .collect::<Vec<_>>();
    lines.push(format!("|{}|", cells.join("|")));
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use mcd_core::package::MCD_MIMETYPE;

    use super::*;

    #[test]
    fn queries_aggregate_values() {
        let package = package();
        let result = query_package(
            &package,
            "select count(*) as rows, max(revenue_gbp) as max_revenue from revenue",
        )
        .expect("query succeeds");

        assert_eq!(result.columns, ["rows", "max_revenue"]);
        assert_eq!(result.row_count(), 1);
        assert_eq!(result.rows[0][0], QueryValue::Integer(2));
        assert_eq!(result.rows[0][1], QueryValue::Real(142500.0));
    }

    #[test]
    fn rejects_writes() {
        let package = package();
        let err = query_package(&package, "delete from revenue").expect_err("write rejected");

        assert!(err.to_string().contains("query must be a SELECT statement"));
    }

    fn package() -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","tables":[{"id":"revenue","data":"tables/revenue.csv","schema":"tables/revenue.schema.json"}]}"#,
            ),
            ("content/main.md", "# Report\n"),
            (
                "tables/revenue.schema.json",
                r#"{"id":"revenue","columns":[{"name":"quarter","type":"string"},{"name":"revenue_gbp","type":"decimal"}]}"#,
            ),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\nQ2,142500.00\n"),
        ]))
        .expect("package opens")
    }

    fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(*path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip").into_inner()
    }
}
