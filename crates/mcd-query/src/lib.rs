//! Read-only SQL querying for MCD package tables.

use std::path::Path;

use anyhow::{Context, Result, bail};
use mcd_core::{
    Manifest, McdPackage,
    schema::{ColumnType, TableColumnSchema},
    tables::{DataTable, TypedValue, load_manifest_tables},
};
use rusqlite::{
    Connection, params, params_from_iter,
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
    load_tables_into_sqlite(&mut connection, &manifest, tables.values())?;
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

const METADATA_TABLES: &[&str] = &[
    "mcd_tables",
    "mcd_columns",
    "mcd_primary_keys",
    "mcd_foreign_keys",
    "mcd_units",
];

fn load_tables_into_sqlite<'a>(
    connection: &mut Connection,
    manifest: &Manifest,
    tables: impl IntoIterator<Item = &'a mcd_core::tables::DataTable>,
) -> Result<()> {
    let tables = tables.into_iter().collect::<Vec<_>>();
    reject_metadata_name_collisions(&tables)?;

    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    let transaction = connection.transaction()?;
    create_metadata_tables(&transaction)?;
    insert_metadata(&transaction, manifest, &tables)?;

    for table in &tables {
        create_data_table(&transaction, table)?;
    }

    for table in &tables {
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

fn reject_metadata_name_collisions(tables: &[&DataTable]) -> Result<()> {
    for table in tables {
        if METADATA_TABLES.contains(&table.id.as_str()) {
            bail!(
                "table id '{}' is reserved for MCD SQL metadata introspection",
                table.id
            );
        }
    }
    Ok(())
}

fn create_data_table(transaction: &rusqlite::Transaction<'_>, table: &DataTable) -> Result<()> {
    let mut definitions = table
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
        .collect::<Vec<_>>();

    if !table.schema.primary_key.is_empty() {
        definitions.push(format!(
            "PRIMARY KEY ({})",
            quote_identifiers(&table.schema.primary_key)
        ));
    }

    for foreign_key in &table.schema.foreign_keys {
        definitions.push(format!(
            "FOREIGN KEY ({}) REFERENCES {} ({})",
            quote_identifiers(&foreign_key.columns),
            quote_identifier(&foreign_key.references.table),
            quote_identifiers(&foreign_key.references.columns)
        ));
    }

    transaction.execute(
        &format!(
            "CREATE TABLE {} ({})",
            quote_identifier(&table.id),
            definitions.join(", ")
        ),
        [],
    )?;
    Ok(())
}

fn create_metadata_tables(transaction: &rusqlite::Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE mcd_tables (
            table_id TEXT PRIMARY KEY,
            data_path TEXT NOT NULL,
            schema_path TEXT NOT NULL
        );
        CREATE TABLE mcd_columns (
            table_id TEXT NOT NULL,
            column_name TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            type TEXT NOT NULL,
            label TEXT,
            nullable INTEGER NOT NULL,
            enum_values TEXT,
            unit_code TEXT,
            unit_label TEXT,
            unit_custom INTEGER NOT NULL,
            PRIMARY KEY (table_id, column_name)
        );
        CREATE TABLE mcd_primary_keys (
            table_id TEXT NOT NULL,
            column_name TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            PRIMARY KEY (table_id, ordinal)
        );
        CREATE TABLE mcd_foreign_keys (
            table_id TEXT NOT NULL,
            column_name TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            ref_table_id TEXT NOT NULL,
            ref_column_name TEXT NOT NULL,
            PRIMARY KEY (table_id, column_name, ref_table_id, ref_column_name)
        );
        CREATE TABLE mcd_units (
            table_id TEXT NOT NULL,
            column_name TEXT NOT NULL,
            unit_code TEXT,
            unit_label TEXT,
            unit_custom INTEGER NOT NULL,
            PRIMARY KEY (table_id, column_name)
        );
        "#,
    )?;
    Ok(())
}

fn insert_metadata(
    transaction: &rusqlite::Transaction<'_>,
    manifest: &Manifest,
    tables: &[&DataTable],
) -> Result<()> {
    for table in tables {
        let entry = manifest
            .tables
            .iter()
            .find(|entry| entry.id == table.id)
            .with_context(|| format!("missing manifest entry for table '{}'", table.id))?;
        transaction.execute(
            "INSERT INTO mcd_tables (table_id, data_path, schema_path) VALUES (?, ?, ?)",
            params![entry.id, entry.data, entry.schema],
        )?;

        for (index, column) in table.schema.columns.iter().enumerate() {
            insert_column_metadata(transaction, &table.id, index, column)?;
        }

        for (index, column) in table.schema.primary_key.iter().enumerate() {
            transaction.execute(
                "INSERT INTO mcd_primary_keys (table_id, column_name, ordinal) VALUES (?, ?, ?)",
                params![table.id, column, index as i64 + 1],
            )?;
        }

        for foreign_key in &table.schema.foreign_keys {
            for (index, (column, referenced_column)) in foreign_key
                .columns
                .iter()
                .zip(foreign_key.references.columns.iter())
                .enumerate()
            {
                transaction.execute(
                    "INSERT INTO mcd_foreign_keys (table_id, column_name, ordinal, ref_table_id, ref_column_name) VALUES (?, ?, ?, ?, ?)",
                    params![
                        table.id,
                        column,
                        index as i64 + 1,
                        foreign_key.references.table,
                        referenced_column
                    ],
                )?;
            }
        }
    }
    Ok(())
}

fn insert_column_metadata(
    transaction: &rusqlite::Transaction<'_>,
    table_id: &str,
    index: usize,
    column: &TableColumnSchema,
) -> Result<()> {
    let enum_values = if column.enum_values.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&column.enum_values)?)
    };
    let unit_code = column.unit.as_ref().and_then(|unit| unit.code.as_deref());
    let unit_label = column.unit.as_ref().and_then(|unit| unit.label.as_deref());
    let unit_custom = column.unit.as_ref().is_some_and(|unit| unit.custom);

    transaction.execute(
        "INSERT INTO mcd_columns (table_id, column_name, ordinal, type, label, nullable, enum_values, unit_code, unit_label, unit_custom) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            table_id,
            column.name,
            index as i64 + 1,
            column_type_name(column.value_type),
            column.label,
            i64::from(column.nullable),
            enum_values,
            unit_code,
            unit_label,
            i64::from(unit_custom),
        ],
    )?;

    if column.unit.is_some() {
        transaction.execute(
            "INSERT INTO mcd_units (table_id, column_name, unit_code, unit_label, unit_custom) VALUES (?, ?, ?, ?, ?)",
            params![table_id, column.name, unit_code, unit_label, i64::from(unit_custom)],
        )?;
    }
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

fn column_type_name(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "string",
        ColumnType::Integer => "integer",
        ColumnType::Decimal => "decimal",
        ColumnType::Boolean => "boolean",
        ColumnType::Date => "date",
        ColumnType::Datetime => "datetime",
        ColumnType::Time => "time",
        ColumnType::Enum => "enum",
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

fn quote_identifiers(identifiers: &[String]) -> String {
    identifiers
        .iter()
        .map(|identifier| quote_identifier(identifier))
        .collect::<Vec<_>>()
        .join(", ")
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

    #[test]
    fn exposes_mcd_schema_metadata_to_sql() {
        let package = related_package();

        let primary_keys = query_package(
            &package,
            "select table_id, column_name, ordinal from mcd_primary_keys order by table_id",
        )
        .expect("primary keys query succeeds");
        assert_eq!(
            primary_keys.rows,
            vec![
                vec![
                    QueryValue::Text("customers".to_owned()),
                    QueryValue::Text("customer_id".to_owned()),
                    QueryValue::Integer(1),
                ],
                vec![
                    QueryValue::Text("orders".to_owned()),
                    QueryValue::Text("order_id".to_owned()),
                    QueryValue::Integer(1),
                ],
            ]
        );

        let foreign_keys = query_package(
            &package,
            "select table_id, column_name, ref_table_id, ref_column_name from mcd_foreign_keys",
        )
        .expect("foreign keys query succeeds");
        assert_eq!(
            foreign_keys.rows,
            vec![vec![
                QueryValue::Text("orders".to_owned()),
                QueryValue::Text("customer_id".to_owned()),
                QueryValue::Text("customers".to_owned()),
                QueryValue::Text("customer_id".to_owned()),
            ]]
        );

        let units = query_package(
            &package,
            "select table_id, column_name, unit_code, unit_label from mcd_units",
        )
        .expect("units query succeeds");
        assert_eq!(
            units.rows,
            vec![vec![
                QueryValue::Text("orders".to_owned()),
                QueryValue::Text("amount".to_owned()),
                QueryValue::Text("GBP".to_owned()),
                QueryValue::Text("GBP".to_owned()),
            ]]
        );
    }

    #[test]
    fn creates_sqlite_key_constraints_for_pragma_introspection() {
        let package = related_package();

        let table_info = query_package(
            &package,
            "select name, pk from pragma_table_info('customers') where pk > 0",
        )
        .expect("pragma table_info query succeeds");
        assert_eq!(
            table_info.rows,
            vec![vec![
                QueryValue::Text("customer_id".to_owned()),
                QueryValue::Integer(1),
            ]]
        );

        let foreign_key_info = query_package(
            &package,
            "select [table], [from], [to] from pragma_foreign_key_list('orders')",
        )
        .expect("pragma foreign_key_list query succeeds");
        assert_eq!(
            foreign_key_info.rows,
            vec![vec![
                QueryValue::Text("customers".to_owned()),
                QueryValue::Text("customer_id".to_owned()),
                QueryValue::Text("customer_id".to_owned()),
            ]]
        );
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

    fn related_package() -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","tables":[
                    {"id":"customers","data":"tables/customers.csv","schema":"tables/customers.schema.json"},
                    {"id":"orders","data":"tables/orders.csv","schema":"tables/orders.schema.json"}
                ]}"#,
            ),
            ("content/main.md", "# Orders\n"),
            (
                "tables/customers.schema.json",
                r#"{"id":"customers","primaryKey":["customer_id"],"columns":[
                    {"name":"customer_id","type":"string"},
                    {"name":"name","type":"string"}
                ]}"#,
            ),
            ("tables/customers.csv", "customer_id,name\nc1,Alice\n"),
            (
                "tables/orders.schema.json",
                r#"{"id":"orders","primaryKey":["order_id"],"foreignKeys":[{
                    "columns":["customer_id"],
                    "references":{"table":"customers","columns":["customer_id"]}
                }],"columns":[
                    {"name":"order_id","type":"string"},
                    {"name":"customer_id","type":"string"},
                    {"name":"amount","type":"decimal","unit":{"code":"GBP","label":"GBP"}}
                ]}"#,
            ),
            ("tables/orders.csv", "order_id,customer_id,amount\no1,c1,12.50\n"),
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
