use std::{fs, path::Path};

use anyhow::{Context, Result};
use mcd_core::pdf::{PdfConversionOptions, pdf_to_mcd_bytes};

pub fn run(input: &Path, output: &Path, title: Option<&str>) -> Result<()> {
    let pdf = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let mcd = pdf_to_mcd_bytes(
        &pdf,
        PdfConversionOptions {
            title: title.map(ToOwned::to_owned),
            source_filename: input
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned),
        },
    )?;
    fs::write(output, mcd).with_context(|| format!("write {}", output.display()))?;
    Ok(())
}
