//! MCD block directive parsing.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    document::SourceSpan,
    errors::{Diagnostic, McdError, Result},
};

/// Directive parsing options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectiveParseOptions {
    /// Reject unknown fields.
    pub strict: bool,
}

impl Default for DirectiveParseOptions {
    fn default() -> Self {
        Self { strict: true }
    }
}

/// Table directive display mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableDisplay {
    /// Render as a table.
    #[default]
    Table,
    /// Render as a chart backed by table data.
    Chart,
}

/// Parsed `:::table` placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePlacement {
    /// Optional placement reference.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// Referenced table id.
    pub table: String,
    /// Optional referenced view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    /// Placement display mode.
    #[serde(default)]
    pub display: TableDisplay,
    /// Optional caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Optional numbering mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<String>,
}

/// Parsed `:::image` placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePlacement {
    /// Optional placement reference.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// Optional direct asset reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    /// Optional image metadata reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Optional caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Optional alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// Parse a `:::table` directive body.
pub fn parse_table_directive(
    body: &str,
    source: Option<SourceSpan>,
    options: DirectiveParseOptions,
) -> Result<TablePlacement> {
    let fields = parse_fields(
        body,
        &["ref", "table", "view", "display", "caption", "numbering"],
        source,
        options,
    )?;

    let table = required_field(&fields, "table", "directive.table.table.missing", source)?;
    let display = match fields.get("display").map(String::as_str).unwrap_or("table") {
        "table" => TableDisplay::Table,
        "chart" => TableDisplay::Chart,
        value => {
            return Err(directive_error(
                "directive.table.display.invalid",
                format!("Table directive display must be 'table' or 'chart', got '{value}'."),
                source,
            ));
        }
    };

    if display == TableDisplay::Chart && empty_or_missing(fields.get("view")) {
        return Err(directive_error(
            "directive.table.view.required",
            "Table directives with display: chart must include view.",
            source,
        ));
    }

    Ok(TablePlacement {
        ref_id: optional_field(&fields, "ref"),
        table,
        view: optional_field(&fields, "view"),
        display,
        caption: optional_field(&fields, "caption"),
        numbering: optional_field(&fields, "numbering"),
    })
}

/// Parse an `:::image` directive body.
pub fn parse_image_directive(
    body: &str,
    source: Option<SourceSpan>,
    options: DirectiveParseOptions,
) -> Result<ImagePlacement> {
    let fields = parse_fields(
        body,
        &["ref", "asset", "image", "caption", "alt"],
        source,
        options,
    )?;

    let asset = optional_field(&fields, "asset");
    let image = optional_field(&fields, "image");
    if asset.is_none() && image.is_none() {
        return Err(directive_error(
            "directive.image.asset.missing",
            "Image directive must include asset or image.",
            source,
        ));
    }

    Ok(ImagePlacement {
        ref_id: optional_field(&fields, "ref"),
        asset,
        image,
        caption: optional_field(&fields, "caption"),
        alt: optional_field(&fields, "alt"),
    })
}

fn parse_fields(
    body: &str,
    known_fields: &[&str],
    source: Option<SourceSpan>,
    options: DirectiveParseOptions,
) -> Result<HashMap<String, String>> {
    let known = known_fields.iter().copied().collect::<HashSet<_>>();
    let mut fields = HashMap::new();

    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            return Err(directive_error(
                "directive.syntax.invalid",
                format!(
                    "Directive field on body line {} must use 'key: value' syntax.",
                    index + 1
                ),
                source,
            ));
        };
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(directive_error(
                "directive.syntax.invalid",
                format!(
                    "Directive field on body line {} has an invalid key.",
                    index + 1
                ),
                source,
            ));
        }
        if options.strict && !known.contains(key) {
            return Err(directive_error(
                "directive.field.unknown",
                format!("Unknown directive field '{key}'."),
                source,
            ));
        }
        if fields
            .insert(key.to_string(), value.trim().to_string())
            .is_some()
        {
            return Err(directive_error(
                "directive.field.duplicate",
                format!("Duplicate directive field '{key}'."),
                source,
            ));
        }
    }

    Ok(fields)
}

fn required_field(
    fields: &HashMap<String, String>,
    name: &'static str,
    code: &'static str,
    source: Option<SourceSpan>,
) -> Result<String> {
    optional_field(fields, name).ok_or_else(|| {
        directive_error(
            code,
            format!("Directive field '{name}' is required."),
            source,
        )
    })
}

fn optional_field(fields: &HashMap<String, String>, name: &str) -> Option<String> {
    fields
        .get(name)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn empty_or_missing(value: Option<&String>) -> bool {
    value.is_none_or(|value| value.trim().is_empty())
}

fn directive_error(
    code: impl Into<String>,
    message: impl Into<String>,
    source: Option<SourceSpan>,
) -> McdError {
    let diagnostic = match source {
        Some(source) => Diagnostic::error(code, message).with_source(source.to_string()),
        None => Diagnostic::error(code, message),
    };
    McdError::from_diagnostic(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_display_defaults_to_table() {
        let placement = parse_table_directive(
            "ref: revenue-table\ntable: revenue\ncaption: Revenue",
            None,
            DirectiveParseOptions::default(),
        )
        .expect("directive parses");

        assert_eq!(placement.display, TableDisplay::Table);
        assert_eq!(placement.table, "revenue");
    }

    #[test]
    fn chart_requires_view() {
        let err = parse_table_directive(
            "table: revenue\ndisplay: chart",
            None,
            DirectiveParseOptions::default(),
        )
        .expect_err("chart without view should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("directive.table.view.required")
        );
    }

    #[test]
    fn image_requires_asset_or_metadata_ref() {
        let err = parse_image_directive(
            "ref: process-diagram",
            None,
            DirectiveParseOptions::default(),
        )
        .expect_err("image without asset should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("directive.image.asset.missing")
        );
    }
}
