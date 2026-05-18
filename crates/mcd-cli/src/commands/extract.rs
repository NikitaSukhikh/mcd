use std::path::Path;

use anyhow::{Result, bail};
use mcd_core::{McdPackage, export::json_export};

pub fn run(
    file: &Path,
    json: bool,
    markdown: bool,
    expand_tables: bool,
    tables: bool,
) -> Result<()> {
    let package = McdPackage::open_path(file)?;
    let manifest = package.manifest()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&json_export(&package)?)?);
        return Ok(());
    }

    if markdown {
        if expand_tables {
            bail!("expanded Markdown export is not implemented yet");
        }
        println!("{}", package.read_to_string(&manifest.entrypoint)?);
        return Ok(());
    }

    if tables {
        bail!("table extraction is not implemented yet");
    }

    bail!("choose one extraction mode: --json, --markdown, or --tables");
}
