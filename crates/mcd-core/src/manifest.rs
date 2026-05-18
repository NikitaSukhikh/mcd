//! Manifest parsing and basic validation.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    errors::{Diagnostic, McdError, Result},
    package::validate_internal_path,
};

/// Root `manifest.json` model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format name. Must be `MCD`.
    pub format: String,
    /// Format version. Alpha currently supports `0.1`.
    pub version: String,
    /// Conformance profile.
    pub profile: McdProfile,
    /// Markdown entrypoint path.
    pub entrypoint: String,
    /// Optional title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional encoding declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// Declared tables.
    #[serde(default)]
    pub tables: Vec<TableManifestEntry>,
    /// Optional layout file paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutManifestEntry>,
}

impl Manifest {
    /// Parse a manifest from JSON bytes and validate required basics.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let manifest: Self = serde_json::from_slice(bytes)?;
        manifest.validate_basic()?;
        Ok(manifest)
    }

    /// Basic manifest validation that does not require package file existence checks.
    pub fn validate_basic(&self) -> Result<()> {
        if self.format != "MCD" {
            return Err(McdError::from_diagnostic(
                Diagnostic::error(
                    "manifest.format.unsupported",
                    "Manifest format must be MCD.",
                )
                .with_source("manifest.json"),
            ));
        }

        if self.version != "0.1" {
            return Err(McdError::from_diagnostic(
                Diagnostic::error(
                    "manifest.version.unsupported",
                    "Manifest version must be 0.1 for this alpha parser.",
                )
                .with_source("manifest.json"),
            ));
        }

        validate_manifest_path("manifest.entrypoint.invalid", &self.entrypoint)?;

        let mut ids = std::collections::HashSet::new();
        for table in &self.tables {
            if table.id.trim().is_empty() {
                return Err(McdError::from_diagnostic(
                    Diagnostic::error("manifest.table.id.empty", "Table id cannot be empty.")
                        .with_source("manifest.json"),
                ));
            }
            if !ids.insert(table.id.clone()) {
                return Err(McdError::from_diagnostic(
                    Diagnostic::error(
                        "manifest.table.id.duplicate",
                        format!("Duplicate table id '{}'.", table.id),
                    )
                    .with_source("manifest.json"),
                ));
            }
            validate_manifest_path("manifest.table.data.invalid", &table.data)?;
            validate_manifest_path("manifest.table.schema.invalid", &table.schema)?;
            for path in table.views.values() {
                validate_manifest_path("manifest.table.view.invalid", path)?;
            }
        }

        if let Some(layout) = &self.layout {
            if let Some(styles) = &layout.styles {
                validate_manifest_path("manifest.layout.styles.invalid", styles)?;
            }
            if let Some(page_map) = &layout.page_map {
                validate_manifest_path("manifest.layout.page_map.invalid", page_map)?;
            }
        }

        Ok(())
    }
}

fn validate_manifest_path(code: &'static str, path: &str) -> Result<()> {
    validate_internal_path(path).map(|_| ()).map_err(|_| {
        McdError::from_diagnostic(
            Diagnostic::error(code, format!("Invalid internal package path '{path}'."))
                .with_source("manifest.json"),
        )
    })
}

/// Supported MCD conformance profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McdProfile {
    /// Semantic source profile.
    #[serde(rename = "MCD-Core")]
    Core,
    /// Rendered profile.
    #[serde(rename = "MCD-Rendered")]
    Rendered,
    /// Verified profile.
    #[serde(rename = "MCD-Verified")]
    Verified,
    /// Signed profile.
    #[serde(rename = "MCD-Signed")]
    Signed,
}

/// Manifest declaration for a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableManifestEntry {
    /// Stable table id.
    pub id: String,
    /// CSV data path.
    pub data: String,
    /// Table schema JSON path.
    pub schema: String,
    /// Named table views.
    #[serde(default)]
    pub views: IndexMap<String, String>,
}

/// Optional layout paths in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutManifestEntry {
    /// Styles JSON path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub styles: Option<String>,
    /// Page map JSON path.
    #[serde(default, rename = "pageMap", skip_serializing_if = "Option::is_none")]
    pub page_map: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let manifest = Manifest::from_slice(
            br#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#,
        )
        .expect("manifest parses");

        assert_eq!(manifest.entrypoint, "content/main.md");
        assert!(manifest.tables.is_empty());
    }

    #[test]
    fn rejects_duplicate_table_ids() {
        let err = Manifest::from_slice(
            br#"{
                "format":"MCD",
                "version":"0.1",
                "profile":"MCD-Core",
                "entrypoint":"content/main.md",
                "tables":[
                    {"id":"revenue","data":"tables/a.csv","schema":"tables/a.schema.json"},
                    {"id":"revenue","data":"tables/b.csv","schema":"tables/b.schema.json"}
                ]
            }"#,
        )
        .expect_err("duplicate ids should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("manifest.table.id.duplicate")
        );
    }
}
