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
    if html && should_write_html_project(output) {
        let rendered = mcd_render::render_html_project(&package)?;
        fs::create_dir_all(output)?;
        fs::write(output.join("index.html"), rendered.index_html)?;
        fs::write(output.join("styles.css"), rendered.styles_css)?;
        fs::create_dir_all(output.join("assets"))?;
        for asset in rendered.assets {
            let path = output.join(asset.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, asset.bytes)?;
        }
        return Ok(());
    }

    let rendered = if html {
        mcd_render::render_html(&package)?
    } else {
        expanded_markdown_export(&package)?
    };
    fs::write(output, rendered)?;
    Ok(())
}

fn should_write_html_project(output: &Path) -> bool {
    output.is_dir() || output.extension().is_none()
}
