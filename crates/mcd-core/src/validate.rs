//! Cross-file validation entry points.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    McdPackage,
    annotations::{load_manifest_annotations, validate_annotation_markers},
    directives::TableDisplay,
    document::{DocumentBlock, McdDocument, SourceSpan},
    errors::{Diagnostic, McdError},
    images::{load_manifest_images, validate_image_anchors},
    table_view::TableView,
    tables::{DataTable, load_manifest_tables},
};

/// Result of validating a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the package is valid.
    pub valid: bool,
    /// Structured diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    /// Construct a successful validation result.
    #[must_use]
    pub fn valid() -> Self {
        Self {
            valid: true,
            diagnostics: Vec::new(),
        }
    }
}

/// Validate package-level, manifest, Markdown, table, schema, and table view rules.
pub fn validate_package(package: &McdPackage) -> crate::Result<ValidationResult> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = load_manifest_tables(package, &manifest)?;
    let views = load_and_validate_views(package, &manifest, &tables)?;
    validate_table_anchors(&document, &tables, &views)?;
    let images = load_manifest_images(package, &manifest)?;
    validate_image_anchors(&document, &images)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    validate_annotation_markers(&document, &annotations)?;
    Ok(ValidationResult::valid())
}

fn load_and_validate_views(
    package: &McdPackage,
    manifest: &crate::Manifest,
    tables: &IndexMap<String, DataTable>,
) -> crate::Result<IndexMap<String, IndexMap<String, TableView>>> {
    let mut all_views = IndexMap::new();

    for table_entry in &manifest.tables {
        let table = tables.get(&table_entry.id).ok_or_else(|| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "table.internal.missing",
                    format!(
                        "Loaded table '{}' was not available for view validation.",
                        table_entry.id
                    ),
                )
                .with_source("manifest.json"),
            )
        })?;
        let mut table_views = IndexMap::new();

        for (view_id, view_path) in &table_entry.views {
            let view = TableView::from_package(package, view_path)?;
            view.validate(view_id, &table_entry.id, &table.schema, view_path)?;
            table_views.insert(view_id.clone(), view);
        }

        all_views.insert(table_entry.id.clone(), table_views);
    }

    Ok(all_views)
}

fn validate_table_anchors(
    document: &McdDocument,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
) -> crate::Result<()> {
    for block in &document.blocks {
        let DocumentBlock::TableRef {
            placement, source, ..
        } = block
        else {
            continue;
        };

        if !tables.contains_key(&placement.table) {
            return Err(anchor_error(
                "table.anchor.unresolved",
                format!(
                    "Table anchor references undeclared table '{}'.",
                    placement.table
                ),
                document,
                *source,
            ));
        }

        let Some(view_id) = &placement.view else {
            if placement.display == TableDisplay::Chart {
                return Err(anchor_error(
                    "chart.view.missing",
                    "Chart table anchors must reference a chart view.",
                    document,
                    *source,
                ));
            }
            continue;
        };

        let Some(table_views) = views.get(&placement.table) else {
            return Err(anchor_error(
                "view.anchor.unresolved",
                format!(
                    "Table anchor references view '{}' but table '{}' declares no views.",
                    view_id, placement.table
                ),
                document,
                *source,
            ));
        };
        let Some(view) = table_views.get(view_id) else {
            return Err(anchor_error(
                if placement.display == TableDisplay::Chart {
                    "chart.view.unresolved"
                } else {
                    "view.anchor.unresolved"
                },
                format!(
                    "Table anchor references unknown view '{}' for table '{}'.",
                    view_id, placement.table
                ),
                document,
                *source,
            ));
        };

        if placement.display == TableDisplay::Chart && view.display != TableDisplay::Chart {
            return Err(anchor_error(
                "chart.view.not_chart",
                format!(
                    "Chart anchor references view '{}' but that view is not a chart view.",
                    view_id
                ),
                document,
                *source,
            ));
        }
        if placement.display == TableDisplay::Table && view.display != TableDisplay::Table {
            return Err(anchor_error(
                "view.display.mismatch",
                format!(
                    "Table anchor references view '{}' but that view is a chart view.",
                    view_id
                ),
                document,
                *source,
            ));
        }
        if view.table != placement.table {
            return Err(anchor_error(
                "chart.table.mismatch",
                format!(
                    "View '{}' references table '{}' but anchor references table '{}'.",
                    view_id, view.table, placement.table
                ),
                document,
                *source,
            ));
        }
    }

    Ok(())
}

fn anchor_error(
    code: impl Into<String>,
    message: impl Into<String>,
    document: &McdDocument,
    source: Option<SourceSpan>,
) -> McdError {
    let source = source
        .map(|span| format!("{}:{span}", document.source_path))
        .unwrap_or_else(|| document.source_path.clone());
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn validates_table_and_chart_package() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            r#"{"id":"revenue","columns":[
                {"name":"quarter","type":"string","nullable":false},
                {"name":"revenue_gbp","type":"decimal","nullable":false}
            ]}"#,
            r#"{"id":"default","table":"revenue","display":"table","columns":[{"name":"quarter"}]}"#,
            r#"{"id":"chart","table":"revenue","display":"chart","chart":{
                "type":"bar",
                "x":{"column":"quarter"},
                "y":{"column":"revenue_gbp"}
            }}"#,
            Some(
                ":::table\nref: t\ntable: revenue\nview: default\n:::\n\n:::table\nref: c\ntable: revenue\nview: chart\ndisplay: chart\n:::\n",
            ),
        );

        validate_package(&package).expect("valid package");
    }

    #[test]
    fn rejects_csv_header_mismatch() {
        let package = package_with(
            "quarter,amount\nQ1,125000.00\n",
            r#"{"id":"revenue","columns":[
                {"name":"quarter","type":"string"},
                {"name":"revenue_gbp","type":"decimal"}
            ]}"#,
            r#"{"id":"default","table":"revenue","columns":[{"name":"quarter"}]}"#,
            chart_view(),
            None,
        );

        let err = validate_package(&package).expect_err("package should be invalid");
        let diagnostic = err.diagnostic().expect("structured diagnostic");
        assert_eq!(diagnostic.level, crate::errors::DiagnosticLevel::Error);
        assert_eq!(diagnostic.code, "csv.header.mismatch");
        assert!(
            diagnostic
                .message
                .contains("CSV header does not match table schema")
        );
        assert_eq!(diagnostic.source.as_deref(), Some("tables/revenue.csv:1"));
        assert_eq!(diagnostic.related, vec!["tables/revenue.schema.json"]);
    }

    #[test]
    fn rejects_unresolved_table_anchor() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            schema(),
            r#"{"id":"default","table":"revenue","columns":[{"name":"quarter"}]}"#,
            chart_view(),
            Some(":::table\ntable: missing\n:::\n"),
        );

        assert_validation_code(&package, "table.anchor.unresolved");
    }

    #[test]
    fn rejects_unknown_view_column() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            schema(),
            r#"{"id":"default","table":"revenue","columns":[{"name":"missing"}]}"#,
            chart_view(),
            None,
        );

        assert_validation_code(&package, "view.column.unknown");
    }

    #[test]
    fn rejects_unknown_chart_column() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            schema(),
            r#"{"id":"default","table":"revenue","columns":[{"name":"quarter"}]}"#,
            r#"{"id":"chart","table":"revenue","display":"chart","chart":{
                "type":"bar",
                "x":{"column":"quarter"},
                "y":{"column":"missing"}
            }}"#,
            None,
        );

        assert_validation_code(&package, "chart.column.unknown");
    }

    #[test]
    fn rejects_incompatible_chart_column_type() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            schema(),
            r#"{"id":"default","table":"revenue","columns":[{"name":"quarter"}]}"#,
            r#"{"id":"chart","table":"revenue","display":"chart","chart":{
                "type":"bar",
                "x":{"column":"revenue_gbp"},
                "y":{"column":"quarter"}
            }}"#,
            None,
        );

        assert_validation_code(&package, "chart.column.type.incompatible");
    }

    #[test]
    fn rejects_chart_anchor_referencing_non_chart_view() {
        let package = package_with(
            "quarter,revenue_gbp\nQ1,125000.00\n",
            schema(),
            r#"{"id":"default","table":"revenue","columns":[{"name":"quarter"}]}"#,
            chart_view(),
            Some(":::table\ntable: revenue\nview: default\ndisplay: chart\n:::\n"),
        );

        assert_validation_code(&package, "chart.view.not_chart");
    }

    #[test]
    fn validates_image_package() {
        let package = image_package(
            safe_svg(),
            image_metadata(
                "diagram",
                r#""alt":"Workflow diagram","caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );

        validate_package(&package).expect("valid image package");
    }

    #[test]
    fn rejects_missing_image_asset() {
        let package = image_package_without_asset(
            image_metadata(
                "diagram",
                r#""alt":"Workflow diagram","caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );

        assert_validation_code(&package, "asset.missing");
    }

    #[test]
    fn rejects_missing_informative_image_alt() {
        let package = image_package(
            safe_svg(),
            image_metadata(
                "informative",
                r#""caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );

        assert_validation_code(&package, "image.alt.missing");
    }

    #[test]
    fn rejects_unresolved_image_anchor() {
        let package = image_package(
            safe_svg(),
            image_metadata(
                "diagram",
                r#""alt":"Workflow diagram","caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: missing-image\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );

        assert_validation_code(&package, "image.anchor.unresolved");
    }

    #[test]
    fn rejects_svg_script_and_external_resource() {
        let script_package = image_package(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#,
            image_metadata(
                "diagram",
                r#""alt":"Workflow diagram","caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );
        assert_validation_code(&script_package, "security.svg.active_content");

        let external_package = image_package(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/x.png"/></svg>"#,
            image_metadata(
                "diagram",
                r#""alt":"Workflow diagram","caption":"Workflow diagram.","hash":null"#,
            ),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images"]"#,
        );
        assert_validation_code(&external_package, "security.svg.external_reference");
    }

    #[test]
    fn rejects_image_only_table_under_strict() {
        let package = image_package(
            safe_svg(),
            r#"{
                "id":"process-diagram",
                "asset":"assets/process-diagram.svg",
                "mediaType":"image/svg+xml",
                "role":"diagram",
                "alt":"Image of a table.",
                "caption":"Table image.",
                "meaningfulContent":{"tableData":true}
            }"#
            .to_owned(),
            ":::image\nasset: process-diagram\n:::\n",
            r#""conformance":["MCD-Core","MCD-Images","MCD-Strict"]"#,
        );

        assert_validation_code(&package, "image.meaningful_content.unlinked");
    }

    fn assert_validation_code(package: &McdPackage, code: &str) {
        let err = validate_package(package).expect_err("package should be invalid");
        assert_eq!(err.diagnostic().map(|d| d.code.as_str()), Some(code));
    }

    fn package_with(
        csv: &str,
        schema_json: &str,
        table_view_json: &str,
        chart_view_json: &str,
        markdown: Option<&str>,
    ) -> McdPackage {
        let markdown = markdown.unwrap_or(
            ":::table\ntable: revenue\nview: default\n:::\n\n:::table\ntable: revenue\nview: chart\ndisplay: chart\n:::\n",
        );
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            ("manifest.json", manifest()),
            ("content/main.md", markdown),
            ("tables/revenue.csv", csv),
            ("tables/revenue.schema.json", schema_json),
            ("tables/revenue.view.json", table_view_json),
            ("tables/revenue.chart.view.json", chart_view_json),
        ]))
        .expect("package opens")
    }

    fn image_package(
        svg: &str,
        image_json: String,
        markdown: &str,
        conformance: &str,
    ) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes_owned(vec![
            ("mimetype", crate::package::MCD_MIMETYPE.to_owned()),
            ("manifest.json", image_manifest(conformance)),
            ("content/main.md", markdown.to_owned()),
            ("assets/process-diagram.svg", svg.to_owned()),
            ("images/process-diagram.image.json", image_json),
        ]))
        .expect("package opens")
    }

    fn image_package_without_asset(
        image_json: String,
        markdown: &str,
        conformance: &str,
    ) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes_owned(vec![
            ("mimetype", crate::package::MCD_MIMETYPE.to_owned()),
            ("manifest.json", image_manifest(conformance)),
            ("content/main.md", markdown.to_owned()),
            ("images/process-diagram.image.json", image_json),
        ]))
        .expect("package opens")
    }

    fn image_manifest(conformance: &str) -> String {
        format!(
            r#"{{
                "format":"MCD",
                "version":"0.1",
                "profile":"MCD-Core",
                {conformance},
                "entrypoint":"content/main.md",
                "images":[{{"id":"process-diagram","metadata":"images/process-diagram.image.json"}}]
            }}"#
        )
    }

    fn image_metadata(role: &str, extra_fields: &str) -> String {
        format!(
            r#"{{
                "id":"process-diagram",
                "asset":"assets/process-diagram.svg",
                "mediaType":"image/svg+xml",
                "role":"{role}",
                {extra_fields}
            }}"#
        )
    }

    fn safe_svg() -> &'static str {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 60"><rect width="120" height="60" fill="#f2f2f2"/><text x="10" y="35">Process</text></svg>"##
    }

    fn manifest() -> &'static str {
        r#"{
            "format":"MCD",
            "version":"0.1",
            "profile":"MCD-Core",
            "entrypoint":"content/main.md",
            "tables":[{
                "id":"revenue",
                "data":"tables/revenue.csv",
                "schema":"tables/revenue.schema.json",
                "views":{
                    "default":"tables/revenue.view.json",
                    "chart":"tables/revenue.chart.view.json"
                }
            }]
        }"#
    }

    fn schema() -> &'static str {
        r#"{"id":"revenue","columns":[
            {"name":"quarter","type":"string"},
            {"name":"revenue_gbp","type":"decimal"}
        ]}"#
    }

    fn chart_view() -> &'static str {
        r#"{"id":"chart","table":"revenue","display":"chart","chart":{
            "type":"bar",
            "x":{"column":"quarter"},
            "y":{"column":"revenue_gbp"}
        }}"#
    }

    fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(*path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip").into_inner()
    }

    fn zip_bytes_owned(entries: Vec<(&str, String)>) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip").into_inner()
    }
}
