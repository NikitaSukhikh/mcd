use std::{fs, path::Path};

use anyhow::{Result, bail};
use mcd_core::{McdPackage, export::expanded_markdown_export};

pub fn run(file: &Path, html: bool, markdown: bool, output: &Path) -> Result<()> {
    let modes = [html, markdown]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if modes != 1 {
        bail!("choose exactly one render target: --html or --markdown");
    }

    let package = McdPackage::open_path(file)?;
    let rendered = if html {
        mcd_render::render_html(&package)?
    } else {
        expanded_markdown_export(&package)?
    };
    fs::write(output, rendered)?;
    Ok(())
}
