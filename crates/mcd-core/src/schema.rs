//! MCD table schema parsing and column type definitions.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    errors::{Diagnostic, McdError, Result},
    package::McdPackage,
};

/// Parsed table schema JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchema {
    /// Stable schema/table id.
    pub id: String,
    /// Columns that uniquely identify rows in this table.
    #[serde(default, rename = "primaryKey", skip_serializing_if = "Vec::is_empty")]
    pub primary_key: Vec<String>,
    /// Foreign-key relationships from this table to other tables.
    #[serde(default, rename = "foreignKeys", skip_serializing_if = "Vec::is_empty")]
    pub foreign_keys: Vec<ForeignKeySchema>,
    /// Ordered table columns.
    pub columns: Vec<TableColumnSchema>,
}

impl TableSchema {
    /// Parse a table schema from a package entry.
    pub fn from_package(package: &McdPackage, path: &str) -> Result<Self> {
        let bytes = package.read(path).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "schema.file.missing",
                    format!("Declared table schema file '{path}' is missing."),
                )
                .with_source(path.to_owned()),
            )
        })?;
        let schema = serde_json::from_slice::<Self>(bytes)?;
        schema.validate(path)?;
        Ok(schema)
    }

    /// Validate schema-level constraints.
    pub fn validate(&self, source: &str) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(schema_error(
                "schema.id.empty",
                "Table schema id cannot be empty.",
                source,
            ));
        }
        if self.columns.is_empty() {
            return Err(schema_error(
                "schema.columns.empty",
                "Table schema must declare at least one column.",
                source,
            ));
        }

        let mut names = std::collections::HashSet::new();
        for column in &self.columns {
            if column.name.trim().is_empty() {
                return Err(schema_error(
                    "schema.column.name.empty",
                    "Table schema column name cannot be empty.",
                    source,
                ));
            }
            if !names.insert(column.name.clone()) {
                return Err(schema_error(
                    "schema.column.name.duplicate",
                    format!("Duplicate schema column '{}'.", column.name),
                    source,
                ));
            }
            if column.value_type == ColumnType::Enum && column.enum_values.is_empty() {
                return Err(schema_error(
                    "schema.enum.values.missing",
                    format!("Enum column '{}' must declare enum values.", column.name),
                    source,
                ));
            }
        }

        if has_duplicates(&self.primary_key) {
            return Err(schema_error(
                "schema.primary_key.column.duplicate",
                "Primary key columns must be unique.",
                source,
            ));
        }
        for key_column in &self.primary_key {
            let Some(column) = self.column(key_column) else {
                return Err(schema_error(
                    "schema.primary_key.column.unknown",
                    format!("Primary key references unknown column '{key_column}'."),
                    source,
                ));
            };
            if column.nullable {
                return Err(schema_error(
                    "schema.primary_key.column.nullable",
                    format!("Primary key column '{key_column}' cannot be nullable."),
                    source,
                ));
            }
        }

        for foreign_key in &self.foreign_keys {
            foreign_key.validate(self, source)?;
        }

        Ok(())
    }

    /// Return a map keyed by column name.
    #[must_use]
    pub fn columns_by_name(&self) -> IndexMap<&str, &TableColumnSchema> {
        self.columns
            .iter()
            .map(|column| (column.name.as_str(), column))
            .collect()
    }

    /// Return true when the schema contains a column.
    #[must_use]
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|column| column.name == name)
    }

    /// Find a column by name.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<&TableColumnSchema> {
        self.columns.iter().find(|column| column.name == name)
    }
}

/// A foreign-key relationship from this table to another table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeySchema {
    /// Local columns in this table.
    pub columns: Vec<String>,
    /// Referenced table and columns.
    pub references: ForeignKeyReference,
}

impl ForeignKeySchema {
    fn validate(&self, schema: &TableSchema, source: &str) -> Result<()> {
        if self.columns.is_empty() {
            return Err(schema_error(
                "schema.foreign_key.columns.empty",
                "Foreign keys must reference at least one local column.",
                source,
            ));
        }
        if self.references.columns.is_empty() {
            return Err(schema_error(
                "schema.foreign_key.references.columns.empty",
                "Foreign keys must reference at least one target column.",
                source,
            ));
        }
        if self.columns.len() != self.references.columns.len() {
            return Err(schema_error(
                "schema.foreign_key.column_count.mismatch",
                "Foreign key local and referenced column counts must match.",
                source,
            ));
        }
        if has_duplicates(&self.columns) {
            return Err(schema_error(
                "schema.foreign_key.column.duplicate",
                "Foreign key local columns must be unique.",
                source,
            ));
        }
        if has_duplicates(&self.references.columns) {
            return Err(schema_error(
                "schema.foreign_key.references.column.duplicate",
                "Foreign key referenced columns must be unique.",
                source,
            ));
        }
        for column in &self.columns {
            if !schema.has_column(column) {
                return Err(schema_error(
                    "schema.foreign_key.column.unknown",
                    format!("Foreign key references unknown local column '{column}'."),
                    source,
                ));
            }
        }
        if self.references.table.trim().is_empty() {
            return Err(schema_error(
                "schema.foreign_key.references.table.empty",
                "Foreign key referenced table cannot be empty.",
                source,
            ));
        }
        Ok(())
    }
}

/// Foreign-key target table and columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyReference {
    /// Referenced manifest table id.
    pub table: String,
    /// Referenced columns in the target table.
    pub columns: Vec<String>,
}

/// One table column schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableColumnSchema {
    /// CSV header and stable column id.
    pub name: String,
    /// Primitive MCD column type.
    #[serde(rename = "type")]
    pub value_type: ColumnType,
    /// Human-readable label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Whether empty CSV cells are allowed.
    #[serde(default)]
    pub nullable: bool,
    /// Allowed values for enum columns.
    #[serde(default, alias = "values", alias = "enumValues")]
    pub enum_values: Vec<String>,
}

/// Supported primitive table types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    /// UTF-8 string value.
    String,
    /// Signed 64-bit integer value.
    Integer,
    /// Decimal value.
    Decimal,
    /// Boolean value.
    Boolean,
    /// ISO date value.
    Date,
    /// ISO datetime value.
    Datetime,
    /// ISO time value.
    Time,
    /// String value constrained to declared members.
    Enum,
}

impl ColumnType {
    /// Return true for numeric types.
    #[must_use]
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Decimal)
    }

    /// Return true for temporal types.
    #[must_use]
    pub fn is_temporal(self) -> bool {
        matches!(self, Self::Date | Self::Datetime | Self::Time)
    }
}

fn schema_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

fn has_duplicates(values: &[String]) -> bool {
    let mut seen = std::collections::HashSet::new();
    values.iter().any(|value| !seen.insert(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_schema_columns() {
        let schema = serde_json::from_str::<TableSchema>(
            r#"{
                "id": "revenue",
                "primaryKey": ["quarter"],
                "columns": [
                    {"name": "quarter", "type": "string"},
                    {"name": "amount", "type": "decimal", "nullable": true}
                ]
            }"#,
        )
        .expect("schema parses");

        assert_eq!(schema.columns[1].value_type, ColumnType::Decimal);
        assert!(schema.columns[1].nullable);
        assert_eq!(schema.primary_key, ["quarter"]);
    }

    #[test]
    fn enum_columns_require_values() {
        let schema = serde_json::from_str::<TableSchema>(
            r#"{"id":"survey","columns":[{"name":"rating","type":"enum"}]}"#,
        )
        .expect("schema parses");
        let err = schema
            .validate("tables/survey.schema.json")
            .expect_err("invalid");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.enum.values.missing")
        );
    }

    #[test]
    fn primary_key_columns_must_exist_and_be_nonnullable() {
        let missing = serde_json::from_str::<TableSchema>(
            r#"{"id":"revenue","primaryKey":["missing"],"columns":[{"name":"quarter","type":"string"}]}"#,
        )
        .expect("schema parses");
        let err = missing
            .validate("tables/revenue.schema.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.primary_key.column.unknown")
        );

        let nullable = serde_json::from_str::<TableSchema>(
            r#"{"id":"revenue","primaryKey":["quarter"],"columns":[{"name":"quarter","type":"string","nullable":true}]}"#,
        )
        .expect("schema parses");
        let err = nullable
            .validate("tables/revenue.schema.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.primary_key.column.nullable")
        );
    }

    #[test]
    fn foreign_key_columns_must_be_well_formed() {
        let schema = serde_json::from_str::<TableSchema>(
            r#"{
                "id":"orders",
                "foreignKeys":[{
                    "columns":["missing"],
                    "references":{"table":"customers","columns":["customer_id"]}
                }],
                "columns":[{"name":"customer_id","type":"string"}]
            }"#,
        )
        .expect("schema parses");
        let err = schema
            .validate("tables/orders.schema.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.foreign_key.column.unknown")
        );
    }

    #[test]
    fn key_columns_must_be_unique() {
        let duplicate_primary_key = serde_json::from_str::<TableSchema>(
            r#"{"id":"revenue","primaryKey":["quarter","quarter"],"columns":[{"name":"quarter","type":"string"}]}"#,
        )
        .expect("schema parses");
        let err = duplicate_primary_key
            .validate("tables/revenue.schema.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.primary_key.column.duplicate")
        );

        let duplicate_foreign_key = serde_json::from_str::<TableSchema>(
            r#"{
                "id":"orders",
                "foreignKeys":[{
                    "columns":["customer_id","customer_id"],
                    "references":{"table":"customers","columns":["customer_id","other_id"]}
                }],
                "columns":[{"name":"customer_id","type":"string"},{"name":"other_id","type":"string"}]
            }"#,
        )
        .expect("schema parses");
        let err = duplicate_foreign_key
            .validate("tables/orders.schema.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("schema.foreign_key.column.duplicate")
        );
    }
}
