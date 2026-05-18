//! Cross-file validation entry points.

use serde::{Deserialize, Serialize};

use crate::{McdPackage, document::McdDocument, errors::Diagnostic};

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

/// Validate package-level, manifest, and Markdown directive rules currently implemented.
pub fn validate_package(package: &McdPackage) -> crate::Result<ValidationResult> {
    let manifest = package.manifest()?;
    let _document = McdDocument::from_package(package, &manifest)?;
    Ok(ValidationResult::valid())
}
