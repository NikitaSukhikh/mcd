//! Table and chart view parsing and validation.

use serde::{Deserialize, Serialize};

use crate::{
    directives::TableDisplay,
    errors::{Diagnostic, McdError, Result},
    package::McdPackage,
    schema::{ColumnType, TableSchema},
};

/// Parsed table view JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableView {
    /// Stable view id.
    pub id: String,
    /// Referenced table id.
    pub table: String,
    /// View display type.
    #[serde(default)]
    pub display: TableDisplay,
    /// Columns included in a table display.
    #[serde(default)]
    pub columns: Vec<ViewColumn>,
    /// Chart specification for chart displays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartSpec>,
    /// Optional style metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<serde_json::Value>,
}

impl TableView {
    /// Parse a table view from a package entry.
    pub fn from_package(package: &McdPackage, path: &str) -> Result<Self> {
        let bytes = package.read(path).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "view.file.missing",
                    format!("Declared table view file '{path}' is missing."),
                )
                .with_source(path.to_owned()),
            )
        })?;
        serde_json::from_slice::<Self>(bytes).map_err(McdError::from)
    }

    /// Validate this view against its table schema.
    pub fn validate(
        &self,
        expected_id: &str,
        table_id: &str,
        schema: &TableSchema,
        source: &str,
    ) -> Result<()> {
        if self.id != expected_id {
            return Err(view_error(
                "view.id.mismatch",
                format!(
                    "View id '{}' does not match manifest view id '{}'.",
                    self.id, expected_id
                ),
                source,
            ));
        }
        if self.table != table_id {
            return Err(view_error(
                "view.table.mismatch",
                format!(
                    "View '{}' references table '{}', but manifest attaches it to '{}'.",
                    self.id, self.table, table_id
                ),
                source,
            ));
        }

        match self.display {
            TableDisplay::Table => self.validate_table_columns(schema, source),
            TableDisplay::Chart => self.validate_chart(schema, source),
        }
    }

    fn validate_table_columns(&self, schema: &TableSchema, source: &str) -> Result<()> {
        for column in &self.columns {
            if !schema.has_column(&column.name) {
                return Err(view_error(
                    "view.column.unknown",
                    format!(
                        "View '{}' references unknown schema column '{}'.",
                        self.id, column.name
                    ),
                    source,
                ));
            }
            validate_format_declarations(
                column.format.as_deref(),
                column.currency.as_deref(),
                column.unit.as_deref(),
                column.percent,
                schema
                    .column(&column.name)
                    .map(|schema_column| schema_column.value_type),
                source,
            )?;
        }
        Ok(())
    }

    fn validate_chart(&self, schema: &TableSchema, source: &str) -> Result<()> {
        let Some(chart) = &self.chart else {
            return Err(view_error(
                "chart.spec.missing",
                format!("Chart view '{}' must include a chart object.", self.id),
                source,
            ));
        };
        chart.validate(schema, source)?;
        Ok(())
    }
}

/// One table view column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewColumn {
    /// Referenced schema column name.
    pub name: String,
    /// Optional display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional format declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Currency code for currency-formatted numeric values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Unit label for numeric values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Whether the value is a percentage.
    #[serde(default)]
    pub percent: bool,
}

/// Constrained chart specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpec {
    /// Chart type in the alpha subset.
    #[serde(rename = "type")]
    pub chart_type: ChartType,
    /// X-axis encoding.
    pub x: ChartEncoding,
    /// Y-axis encoding.
    pub y: ChartEncoding,
    /// Optional series encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<ChartEncoding>,
    /// Optional grouping encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping: Option<ChartEncoding>,
    /// Optional mark label declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_labels: Option<MarkLabels>,
}

impl ChartSpec {
    fn validate(&self, schema: &TableSchema, source: &str) -> Result<()> {
        self.x.validate_known_column(schema, source)?;
        self.y.validate_known_column(schema, source)?;
        if let Some(series) = &self.series {
            series.validate_known_column(schema, source)?;
        }
        if let Some(grouping) = &self.grouping {
            grouping.validate_known_column(schema, source)?;
        }
        if let Some(mark_labels) = &self.mark_labels {
            mark_labels.validate(schema, source)?;
        }

        let y_type = column_type(schema, &self.y.column, source)?;
        if !y_type.is_numeric() {
            return Err(view_error(
                "chart.column.type.incompatible",
                format!(
                    "Chart y column '{}' must be integer or decimal.",
                    self.y.column
                ),
                source,
            ));
        }
        self.y.validate_format(schema, source)?;
        self.x.validate_format(schema, source)?;

        if self.chart_type == ChartType::Scatter {
            let x_type = column_type(schema, &self.x.column, source)?;
            if !x_type.is_numeric() && !x_type.is_temporal() {
                return Err(view_error(
                    "chart.column.type.incompatible",
                    format!(
                        "Scatter chart x column '{}' must be numeric or temporal.",
                        self.x.column
                    ),
                    source,
                ));
            }
        }

        Ok(())
    }
}

/// Supported alpha chart types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartType {
    /// Bar chart.
    Bar,
    /// Line chart.
    Line,
    /// Area chart.
    Area,
    /// Scatter chart.
    Scatter,
}

/// A chart encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartEncoding {
    /// Referenced schema column.
    pub column: String,
    /// Optional display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional value format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Currency code for currency-formatted numeric values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Unit label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Whether the value is a percentage.
    #[serde(default)]
    pub percent: bool,
}

impl ChartEncoding {
    fn validate_known_column(&self, schema: &TableSchema, source: &str) -> Result<()> {
        if schema.has_column(&self.column) {
            Ok(())
        } else {
            Err(view_error(
                "chart.column.unknown",
                format!("Chart references unknown schema column '{}'.", self.column),
                source,
            ))
        }
    }

    fn validate_format(&self, schema: &TableSchema, source: &str) -> Result<()> {
        validate_format_declarations(
            self.format.as_deref(),
            self.currency.as_deref(),
            self.unit.as_deref(),
            self.percent,
            schema.column(&self.column).map(|column| column.value_type),
            source,
        )
    }
}

/// Mark label display and formatting declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkLabels {
    /// Whether labels are shown.
    #[serde(default)]
    pub show: bool,
    /// Optional label source column. Defaults to the chart y column when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    /// Optional value format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Currency code for currency-formatted numeric values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Unit label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Whether the value is a percentage.
    #[serde(default)]
    pub percent: bool,
}

impl MarkLabels {
    fn validate(&self, schema: &TableSchema, source: &str) -> Result<()> {
        if let Some(column) = &self.column {
            if !schema.has_column(column) {
                return Err(view_error(
                    "chart.column.unknown",
                    format!("Chart mark labels reference unknown schema column '{column}'."),
                    source,
                ));
            }
            validate_format_declarations(
                self.format.as_deref(),
                self.currency.as_deref(),
                self.unit.as_deref(),
                self.percent,
                schema
                    .column(column)
                    .map(|schema_column| schema_column.value_type),
                source,
            )?;
        }
        Ok(())
    }
}

fn validate_format_declarations(
    format: Option<&str>,
    currency: Option<&str>,
    _unit: Option<&str>,
    percent: bool,
    column_type: Option<ColumnType>,
    source: &str,
) -> Result<()> {
    if currency.is_some() && format != Some("currency") {
        return Err(view_error(
            "view.format.currency.inconsistent",
            "Currency declarations require format: currency.",
            source,
        ));
    }
    if percent && format.is_some_and(|format| format != "percent") {
        return Err(view_error(
            "view.format.percent.inconsistent",
            "Percent declarations must use format: percent when a format is declared.",
            source,
        ));
    }

    if matches!(format, Some("currency" | "number" | "percent")) {
        let Some(column_type) = column_type else {
            return Ok(());
        };
        if !column_type.is_numeric() {
            return Err(view_error(
                "view.format.type.incompatible",
                "Numeric, currency, and percent formats require integer or decimal columns.",
                source,
            ));
        }
    }

    if matches!(format, Some("date" | "datetime" | "time")) {
        let Some(column_type) = column_type else {
            return Ok(());
        };
        if !column_type.is_temporal() {
            return Err(view_error(
                "view.format.type.incompatible",
                "Date, datetime, and time formats require temporal columns.",
                source,
            ));
        }
    }

    Ok(())
}

fn column_type(schema: &TableSchema, column: &str, source: &str) -> Result<ColumnType> {
    schema
        .column(column)
        .map(|column| column.value_type)
        .ok_or_else(|| {
            view_error(
                "chart.column.unknown",
                format!("Chart references unknown schema column '{column}'."),
                source,
            )
        })
}

fn view_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ColumnType, TableColumnSchema};

    fn schema() -> TableSchema {
        TableSchema {
            id: "revenue".to_owned(),
            primary_key: Vec::new(),
            foreign_keys: Vec::new(),
            columns: vec![
                TableColumnSchema {
                    name: "quarter".to_owned(),
                    value_type: ColumnType::String,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
                TableColumnSchema {
                    name: "amount".to_owned(),
                    value_type: ColumnType::Decimal,
                    label: None,
                    nullable: false,
                    enum_values: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn validates_chart_columns() {
        let view = serde_json::from_str::<TableView>(
            r#"{
                "id":"chart",
                "table":"revenue",
                "display":"chart",
                "chart":{
                    "type":"bar",
                    "x":{"column":"quarter"},
                    "y":{"column":"amount","format":"currency","currency":"GBP"}
                }
            }"#,
        )
        .expect("view parses");

        view.validate("chart", "revenue", &schema(), "tables/chart.view.json")
            .expect("valid chart view");
    }

    #[test]
    fn rejects_unknown_view_column() {
        let view = serde_json::from_str::<TableView>(
            r#"{"id":"default","table":"revenue","columns":[{"name":"missing"}]}"#,
        )
        .expect("view parses");
        let err = view
            .validate("default", "revenue", &schema(), "tables/view.json")
            .expect_err("invalid");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("view.column.unknown")
        );
    }
}
