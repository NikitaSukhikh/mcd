//! CSV table loading and typed value coercion.

use indexmap::IndexMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::{
    Date, OffsetDateTime, PrimitiveDateTime, Time, format_description::well_known::Rfc3339,
};

use crate::{
    errors::{Diagnostic, McdError, Result},
    manifest::{Manifest, TableManifestEntry},
    package::McdPackage,
    schema::{ColumnType, TableColumnSchema, TableSchema},
};

/// Load all manifest-declared tables in manifest order.
pub fn load_manifest_tables(
    package: &McdPackage,
    manifest: &Manifest,
) -> Result<IndexMap<String, DataTable>> {
    let mut tables = IndexMap::new();
    for entry in &manifest.tables {
        let table = DataTable::from_manifest_entry(package, entry)?;
        tables.insert(entry.id.clone(), table);
    }
    Ok(tables)
}

/// A loaded CSV-backed table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataTable {
    /// Stable table id.
    pub id: String,
    /// Package CSV path.
    pub source: String,
    /// Parsed schema.
    pub schema: TableSchema,
    /// Ordered typed rows.
    pub rows: Vec<TableRow>,
}

impl DataTable {
    /// Load and validate a manifest-declared table.
    pub fn from_manifest_entry(package: &McdPackage, entry: &TableManifestEntry) -> Result<Self> {
        if !package.contains(&entry.data) {
            return Err(McdError::from_diagnostic(
                Diagnostic::error(
                    "table.data.missing",
                    format!("Declared table data file '{}' is missing.", entry.data),
                )
                .with_source(entry.data.clone()),
            ));
        }

        let schema = TableSchema::from_package(package, &entry.schema)?;
        if schema.id != entry.id {
            return Err(McdError::from_diagnostic(
                Diagnostic::error(
                    "schema.table.mismatch",
                    format!(
                        "Table schema id '{}' does not match manifest table id '{}'.",
                        schema.id, entry.id
                    ),
                )
                .with_source(entry.schema.clone()),
            ));
        }

        let bytes = package.read(&entry.data)?;
        let rows = load_csv_rows(&entry.id, &entry.data, &entry.schema, bytes, &schema)?;

        Ok(Self {
            id: entry.id.clone(),
            source: entry.data.clone(),
            schema,
            rows,
        })
    }
}

/// A typed table row, keyed by schema column name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    /// Typed cells.
    #[serde(flatten)]
    pub cells: IndexMap<String, TypedValue>,
}

/// A typed table cell value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TypedValue {
    /// Null value from an empty nullable cell.
    Null,
    /// String value.
    String(String),
    /// Integer value.
    Integer(i64),
    /// Decimal value, serialized as a decimal string for stability.
    Decimal(String),
    /// Boolean value.
    Boolean(bool),
    /// Date value.
    Date(String),
    /// Datetime value.
    Datetime(String),
    /// Time value.
    Time(String),
    /// Enum member.
    Enum(String),
}

fn load_csv_rows(
    table_id: &str,
    source: &str,
    schema_source: &str,
    bytes: &[u8],
    schema: &TableSchema,
) -> Result<Vec<TableRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);

    let headers = reader.headers().map_err(|err| {
        McdError::from_diagnostic(
            Diagnostic::error(
                "csv.header.missing",
                format!("CSV table '{table_id}' must include a header row: {err}."),
            )
            .with_source(format!("{source}:1")),
        )
    })?;
    if headers.is_empty() {
        return Err(McdError::from_diagnostic(
            Diagnostic::error(
                "csv.header.missing",
                format!("CSV table '{table_id}' must include a header row."),
            )
            .with_source(format!("{source}:1")),
        ));
    }

    let actual_headers = headers.iter().map(ToOwned::to_owned).collect::<Vec<_>>();
    let expected_headers = schema
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    if actual_headers != expected_headers {
        return Err(McdError::from_diagnostic(
            Diagnostic::error(
                "csv.header.mismatch",
                format!(
                    "CSV header does not match table schema for table '{}'. Expected [{}], got [{}].",
                    table_id,
                    expected_headers.join(", "),
                    actual_headers.join(", ")
                ),
            )
            .with_source(format!("{source}:1"))
            .with_related(schema_source.to_owned()),
        ));
    }

    let mut rows = Vec::new();
    for (record_index, record) in reader.records().enumerate() {
        let record = record.map_err(|err| {
            McdError::from_diagnostic(
                Diagnostic::error("csv.row.invalid", format!("Invalid CSV row: {err}."))
                    .with_source(format!("{source}:{}", record_index + 2)),
            )
        })?;

        let mut cells = IndexMap::new();
        for (column_index, column) in schema.columns.iter().enumerate() {
            let raw = record.get(column_index).unwrap_or_default();
            let value = coerce_cell(raw, column, source, record_index + 2)?;
            cells.insert(column.name.clone(), value);
        }
        rows.push(TableRow { cells });
    }

    Ok(rows)
}

/// Coerce one CSV cell into its schema type.
pub fn coerce_cell(
    raw: &str,
    column: &TableColumnSchema,
    source: &str,
    row_number: usize,
) -> Result<TypedValue> {
    let value = raw.trim();
    if value.is_empty() {
        if column.nullable {
            return Ok(TypedValue::Null);
        }
        return Err(McdError::from_diagnostic(
            Diagnostic::error(
                "csv.cell.empty.nonnullable",
                format!("Column '{}' does not allow empty cells.", column.name),
            )
            .with_source(format!("{source}:{row_number}")),
        ));
    }

    match column.value_type {
        ColumnType::String => Ok(TypedValue::String(raw.to_owned())),
        ColumnType::Integer => value
            .parse::<i64>()
            .map(TypedValue::Integer)
            .map_err(|_| cell_type_error("csv.cell.integer.invalid", column, source, row_number)),
        ColumnType::Decimal => value
            .parse::<Decimal>()
            .map(|decimal| TypedValue::Decimal(decimal.normalize().to_string()))
            .map_err(|_| cell_type_error("csv.cell.decimal.invalid", column, source, row_number)),
        ColumnType::Boolean => parse_bool(value)
            .map(TypedValue::Boolean)
            .ok_or_else(|| cell_type_error("csv.cell.boolean.invalid", column, source, row_number)),
        ColumnType::Date => Date::parse(
            value,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
        .map(|date| TypedValue::Date(date.to_string()))
        .map_err(|_| cell_type_error("csv.cell.date.invalid", column, source, row_number)),
        ColumnType::Datetime => parse_datetime(value)
            .map(TypedValue::Datetime)
            .ok_or_else(|| {
                cell_type_error("csv.cell.datetime.invalid", column, source, row_number)
            }),
        ColumnType::Time => Time::parse(
            value,
            &time::macros::format_description!("[hour]:[minute]:[second]"),
        )
        .or_else(|_| Time::parse(value, &time::macros::format_description!("[hour]:[minute]")))
        .map(|time| time.to_string())
        .map(TypedValue::Time)
        .map_err(|_| cell_type_error("csv.cell.time.invalid", column, source, row_number)),
        ColumnType::Enum => {
            if column.enum_values.iter().any(|allowed| allowed == value) {
                Ok(TypedValue::Enum(value.to_owned()))
            } else {
                Err(McdError::from_diagnostic(
                    Diagnostic::error(
                        "csv.cell.enum.invalid",
                        format!(
                            "Value '{}' is not a member of enum column '{}'.",
                            value, column.name
                        ),
                    )
                    .with_source(format!("{source}:{row_number}")),
                ))
            }
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "TRUE" | "True" => Some(true),
        "false" | "FALSE" | "False" => Some(false),
        _ => None,
    }
}

fn parse_datetime(value: &str) -> Option<String> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(|datetime| datetime.to_string())
        .or_else(|_| {
            PrimitiveDateTime::parse(
                value,
                &time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]"),
            )
            .map(|datetime| datetime.to_string())
        })
        .ok()
}

fn cell_type_error(
    code: &'static str,
    column: &TableColumnSchema,
    source: &str,
    row_number: usize,
) -> McdError {
    McdError::from_diagnostic(
        Diagnostic::error(
            code,
            format!(
                "Cell in column '{}' is not a valid {:?}.",
                column.name, column.value_type
            ),
        )
        .with_source(format!("{source}:{row_number}")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn column(value_type: ColumnType, nullable: bool) -> TableColumnSchema {
        TableColumnSchema {
            name: "value".to_owned(),
            value_type,
            label: None,
            nullable,
            enum_values: Vec::new(),
        }
    }

    #[test]
    fn coerces_decimal() {
        let value = coerce_cell("12.3400", &column(ColumnType::Decimal, false), "t.csv", 2)
            .expect("decimal");
        assert_eq!(value, TypedValue::Decimal("12.34".to_owned()));
    }

    #[test]
    fn rejects_empty_nonnullable_cell() {
        let err =
            coerce_cell("", &column(ColumnType::String, false), "t.csv", 2).expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("csv.cell.empty.nonnullable")
        );
    }

    #[test]
    fn rejects_bad_date() {
        let err = coerce_cell("2026-99-99", &column(ColumnType::Date, false), "t.csv", 2)
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("csv.cell.date.invalid")
        );
    }

    #[test]
    fn rejects_bad_decimal_datetime_time_boolean_and_enum() {
        let cases = [
            (
                "not-money",
                TableColumnSchema {
                    name: "value".to_owned(),
                    value_type: ColumnType::Decimal,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
                "csv.cell.decimal.invalid",
            ),
            (
                "2026-05-18 12:00:00",
                TableColumnSchema {
                    name: "value".to_owned(),
                    value_type: ColumnType::Datetime,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
                "csv.cell.datetime.invalid",
            ),
            (
                "25:00",
                TableColumnSchema {
                    name: "value".to_owned(),
                    value_type: ColumnType::Time,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
                "csv.cell.time.invalid",
            ),
            (
                "yes",
                TableColumnSchema {
                    name: "value".to_owned(),
                    value_type: ColumnType::Boolean,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
                "csv.cell.boolean.invalid",
            ),
            (
                "bronze",
                TableColumnSchema {
                    name: "value".to_owned(),
                    value_type: ColumnType::Enum,
                    label: None,
                    nullable: false,
                    enum_values: vec!["silver".to_owned(), "gold".to_owned()],
                },
                "csv.cell.enum.invalid",
            ),
        ];

        for (raw, column, code) in cases {
            let err = coerce_cell(raw, &column, "t.csv", 2).expect_err("invalid");
            assert_eq!(err.diagnostic().map(|d| d.code.as_str()), Some(code));
        }
    }

    proptest! {
        #[test]
        fn coerces_generated_integers(raw in any::<i64>()) {
            let value = coerce_cell(
                &raw.to_string(),
                &column(ColumnType::Integer, false),
                "t.csv",
                2,
            )
            .expect("integer");
            prop_assert_eq!(value, TypedValue::Integer(raw));
        }
    }
}
