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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_schema_columns() {
        let schema = serde_json::from_str::<TableSchema>(
            r#"{
                "id": "revenue",
                "columns": [
                    {"name": "quarter", "type": "string"},
                    {"name": "amount", "type": "decimal", "nullable": true}
                ]
            }"#,
        )
        .expect("schema parses");

        assert_eq!(schema.columns[1].value_type, ColumnType::Decimal);
        assert!(schema.columns[1].nullable);
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
}
