use std::path::Path;

use anyhow::{Result, bail};
use mcd_core::{
    McdPackage,
    export::{image_export, json_export, table_export},
};

pub fn run(
    file: &Path,
    json: bool,
    markdown: bool,
    expand_tables: bool,
    tables: bool,
    images: bool,
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
        println!(
            "{}",
            serde_json::to_string_pretty(&table_export(&package)?)?
        );
        return Ok(());
    }

    if images {
        println!(
            "{}",
            serde_json::to_string_pretty(&image_export(&package)?)?
        );
        return Ok(());
    }

    bail!("choose one extraction mode: --json, --markdown, --tables, or --images");
}
