//! Core parser, validator, and export APIs for MCD packages.

pub mod directives;
pub mod document;
pub mod errors;
pub mod export;
pub mod manifest;
pub mod markdown;
pub mod package;
pub mod schema;
pub mod table_view;
pub mod tables;
pub mod validate;

pub use errors::{Diagnostic, McdError, Result};
pub use manifest::Manifest;
pub use package::McdPackage;
pub use validate::ValidationResult;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(crate::package::MCD_MIMETYPE, "application/vnd.mcd+zip");
    }
}
