//! Export APIs.

use serde::{Deserialize, Serialize};

use crate::{Manifest, McdPackage, document::McdDocument, tables::DataTable};

/// Canonical JSON export for an MCD package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonExport {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Parsed Markdown document.
    pub document: McdDocument,
}

/// Table extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableExport {
    /// Loaded typed tables in manifest order.
    pub tables: Vec<DataTable>,
}

/// Build the canonical JSON export for a package.
pub fn json_export(package: &McdPackage) -> crate::Result<JsonExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    Ok(JsonExport { manifest, document })
}

/// Build a typed table export for a package.
pub fn table_export(package: &McdPackage) -> crate::Result<TableExport> {
    let manifest = package.manifest()?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    Ok(TableExport {
        tables: tables.into_values().collect(),
    })
}
