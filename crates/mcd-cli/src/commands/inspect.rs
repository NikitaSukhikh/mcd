use std::path::Path;

use anyhow::Result;
use mcd_core::McdPackage;
use serde_json::json;

pub fn run(file: &Path) -> Result<()> {
    let package = McdPackage::open_path(file)?;
    let manifest = package.manifest()?;
    let entry_count = package.entry_paths().len();

    let summary = json!({
        "format": manifest.format,
        "version": manifest.version,
        "profile": manifest.profile,
        "entrypoint": manifest.entrypoint,
        "tables": manifest.tables.len(),
        "entries": entry_count,
    });

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
