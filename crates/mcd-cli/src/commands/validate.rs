use std::path::Path;

use anyhow::Result;
use mcd_core::McdPackage;

use crate::OutputFormat;

pub fn run(file: &Path, format: OutputFormat) -> Result<()> {
    let package = McdPackage::open_path(file)?;
    let manifest = package.manifest()?;

    match format {
        OutputFormat::Text => {
            println!(
                "valid: {} {} ({:?})",
                manifest.format, manifest.version, manifest.profile
            );
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({ "valid": true, "diagnostics": [] })
            );
        }
    }

    Ok(())
}
