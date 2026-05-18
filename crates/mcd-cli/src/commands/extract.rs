use std::path::Path;

use anyhow::{Result, bail};
use mcd_core::{
    McdPackage,
    export::{
        chart_export, expanded_markdown_export, image_export, json_export,
        original_markdown_export, table_export,
    },
};

pub fn run(
    file: &Path,
    json: bool,
    markdown: bool,
    expand_tables: bool,
    tables: bool,
    images: bool,
    charts: bool,
) -> Result<()> {
    let modes = [json, markdown, tables, images, charts]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if modes != 1 {
        bail!(
            "choose exactly one extraction mode: --json, --markdown, --tables, --images, or --charts"
        );
    }
    if expand_tables && !markdown {
        bail!("--expand-tables can only be used with --markdown");
    }

    let package = McdPackage::open_path(file)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&json_export(&package)?)?);
        return Ok(());
    }

    if markdown {
        if expand_tables {
            println!("{}", expanded_markdown_export(&package)?);
            return Ok(());
        }
        println!("{}", original_markdown_export(&package)?);
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

    if charts {
        println!(
            "{}",
            serde_json::to_string_pretty(&chart_export(&package)?)?
        );
        return Ok(());
    }

    Ok(())
}
