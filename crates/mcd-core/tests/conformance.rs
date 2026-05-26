//! Conformance fixture and public JSON schema tests.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use mcd_core::{McdPackage, validate::validate_package};
use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates")
        .parent()
        .expect("repo")
        .to_path_buf()
}

fn schema_path(name: &str) -> PathBuf {
    repo_root().join("schemas").join(name)
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root()
        .join("tests")
        .join("fixtures")
        .join("conformance")
        .join(name)
}

fn load_json(path: &Path) -> Value {
    let bytes = std::fs::read(path).expect("read json");
    serde_json::from_slice(&bytes).expect("parse json")
}

#[test]
fn schemas_are_valid_draft_2020_12_documents() {
    for schema_name in [
        "manifest.schema.json",
        "table.schema.json",
        "table-view.schema.json",
        "image.schema.json",
        "annotation.schema.json",
        "provenance.schema.json",
        "rendering.schema.json",
        "styles.schema.json",
        "page-map.schema.json",
    ] {
        let schema = load_json(&schema_path(schema_name));
        jsonschema::draft202012::meta::validate(&schema)
            .unwrap_or_else(|err| panic!("{schema_name} is not a valid JSON schema: {err}"));
    }
}

#[test]
fn valid_conformance_json_members_match_public_schemas() {
    let schemas = BTreeMap::from([
        (
            "manifest",
            jsonschema::validator_for(&load_json(&schema_path("manifest.schema.json")))
                .expect("manifest schema compiles"),
        ),
        (
            "table",
            jsonschema::validator_for(&load_json(&schema_path("table.schema.json")))
                .expect("table schema compiles"),
        ),
        (
            "table-view",
            jsonschema::validator_for(&load_json(&schema_path("table-view.schema.json")))
                .expect("table view schema compiles"),
        ),
        (
            "image",
            jsonschema::validator_for(&load_json(&schema_path("image.schema.json")))
                .expect("image schema compiles"),
        ),
        (
            "annotation",
            jsonschema::validator_for(&load_json(&schema_path("annotation.schema.json")))
                .expect("annotation schema compiles"),
        ),
        (
            "provenance",
            jsonschema::validator_for(&load_json(&schema_path("provenance.schema.json")))
                .expect("provenance schema compiles"),
        ),
    ]);

    for fixture in valid_fixtures() {
        let package = McdPackage::open_path(fixture_path(fixture)).expect("fixture opens");
        for entry in package.entry_paths() {
            let schema_key = match entry {
                "manifest.json" => Some("manifest"),
                path if path.ends_with(".schema.json") => Some("table"),
                path if path.ends_with(".view.json") => Some("table-view"),
                path if path.ends_with(".image.json") => Some("image"),
                path if path.ends_with(".annotation.json") => Some("annotation"),
                path if path == "provenance/provenance.json"
                    || path.ends_with(".provenance.json") =>
                {
                    Some("provenance")
                }
                _ => None,
            };
            let Some(schema_key) = schema_key else {
                continue;
            };

            let instance = serde_json::from_slice::<Value>(
                package.read(entry).expect("entry should be readable"),
            )
            .unwrap_or_else(|err| panic!("{fixture}:{entry} is not JSON: {err}"));
            schemas[schema_key]
                .validate(&instance)
                .unwrap_or_else(|err| {
                    panic!("{fixture}:{entry} does not match {schema_key} schema: {err}")
                });
        }
    }
}

#[test]
fn conformance_fixtures_validate_as_expected() {
    for fixture in valid_fixtures() {
        let package = McdPackage::open_path(fixture_path(fixture)).expect("fixture opens");
        validate_package(&package).unwrap_or_else(|err| panic!("{fixture} should be valid: {err}"));
    }

    let mut diagnostics = BTreeMap::new();
    for fixture in invalid_fixtures() {
        let err = McdPackage::open_path(fixture_path(fixture))
            .and_then(|package| validate_package(&package))
            .expect_err("invalid fixture should fail");
        let diagnostic = err
            .diagnostic()
            .unwrap_or_else(|| panic!("{fixture} failed without a diagnostic"));
        diagnostics.insert(fixture, diagnostic.code.clone());
    }

    let snapshot = diagnostics
        .into_iter()
        .map(|(fixture, code)| format!("{fixture}: {code}"))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(snapshot, @r#"
invalid-bad-mimetype.mcd: package.mimetype.invalid
invalid-chart-bad-type.mcd: chart.column.type.incompatible
invalid-chart-unknown-column.mcd: chart.column.unknown
invalid-csv-header-mismatch.mcd: csv.header.mismatch
invalid-image-missing-alt.mcd: image.alt.missing
invalid-image-only-table-strict.mcd: image.meaningful_content.unlinked
invalid-missing-manifest.mcd: manifest.missing
invalid-nonnullable-empty-cell.mcd: csv.cell.empty.nonnullable
invalid-path-traversal.mcd: security.path.invalid
invalid-svg-script.mcd: security.svg.active_content
invalid-unresolved-image.mcd: image.anchor.unresolved
invalid-unresolved-table.mcd: table.anchor.unresolved
"#);
}

fn valid_fixtures() -> [&'static str; 7] {
    [
        "valid-minimal.mcd",
        "valid-table.mcd",
        "valid-two-tables.mcd",
        "valid-reused-table.mcd",
        "valid-image.mcd",
        "valid-chart.mcd",
        "valid-image-and-chart.mcd",
    ]
}

fn invalid_fixtures() -> [&'static str; 12] {
    [
        "invalid-missing-manifest.mcd",
        "invalid-bad-mimetype.mcd",
        "invalid-unresolved-table.mcd",
        "invalid-csv-header-mismatch.mcd",
        "invalid-nonnullable-empty-cell.mcd",
        "invalid-unresolved-image.mcd",
        "invalid-image-missing-alt.mcd",
        "invalid-svg-script.mcd",
        "invalid-image-only-table-strict.mcd",
        "invalid-chart-unknown-column.mcd",
        "invalid-chart-bad-type.mcd",
        "invalid-path-traversal.mcd",
    ]
}
