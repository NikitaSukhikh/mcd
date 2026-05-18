//! Export APIs.

use serde::{Deserialize, Serialize};

use crate::{Manifest, McdPackage, document::McdDocument};

/// Canonical JSON export for an MCD package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonExport {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Parsed Markdown document.
    pub document: McdDocument,
}

/// Build the canonical JSON export for a package.
pub fn json_export(package: &McdPackage) -> crate::Result<JsonExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    Ok(JsonExport { manifest, document })
}
