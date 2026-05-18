use std::path::Path;

use anyhow::{Result, bail};
use mcd_core::{McdPackage, validate::validate_package};

use crate::OutputFormat;

pub fn run(file: &Path, format: OutputFormat) -> Result<()> {
    let result = McdPackage::open_path(file).and_then(|package| validate_package(&package));

    match (result, format) {
        (Ok(validation), OutputFormat::Text) => {
            println!("valid");
            debug_assert!(validation.valid);
            Ok(())
        }
        (Ok(validation), OutputFormat::Json) => {
            println!("{}", serde_json::to_string_pretty(&validation)?);
            Ok(())
        }
        (Err(err), OutputFormat::Json) => {
            if let Some(diagnostic) = err.diagnostic() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "valid": false,
                        "diagnostics": [diagnostic],
                    }))?
                );
                bail!("{}", diagnostic.message);
            }
            Err(err.into())
        }
        (Err(err), OutputFormat::Text) => {
            if let Some(diagnostic) = err.diagnostic() {
                bail!("{}: {}", diagnostic.code, diagnostic.message);
            }
            Err(err.into())
        }
    }
}
