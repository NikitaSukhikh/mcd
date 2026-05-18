//! Export APIs.

use serde::{Deserialize, Serialize};

use crate::{
    Manifest, McdPackage, document::McdDocument, images::ImageMetadata, tables::DataTable,
};

/// Canonical JSON export for an MCD package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonExport {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Parsed Markdown document.
    pub document: McdDocument,
    /// Parsed image metadata objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageMetadata>,
}

/// Table extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableExport {
    /// Loaded typed tables in manifest order.
    pub tables: Vec<DataTable>,
}

/// Image metadata extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageExport {
    /// Image metadata objects in manifest order.
    pub images: Vec<ImageMetadata>,
}

/// Build the canonical JSON export for a package.
pub fn json_export(package: &McdPackage) -> crate::Result<JsonExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let images = crate::images::load_manifest_images(package, &manifest)?
        .into_values()
        .collect();
    Ok(JsonExport {
        manifest,
        document,
        images,
    })
}

/// Build a typed table export for a package.
pub fn table_export(package: &McdPackage) -> crate::Result<TableExport> {
    let manifest = package.manifest()?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    Ok(TableExport {
        tables: tables.into_values().collect(),
    })
}

/// Build an image metadata export for a package.
pub fn image_export(package: &McdPackage) -> crate::Result<ImageExport> {
    let manifest = package.manifest()?;
    let images = crate::images::load_manifest_images(package, &manifest)?
        .into_values()
        .collect();
    Ok(ImageExport { images })
}
