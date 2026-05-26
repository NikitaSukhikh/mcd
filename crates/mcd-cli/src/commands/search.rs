use std::path::Path;

use anyhow::Result;
use mcd_core::{McdPackage, SearchKind, SearchOptions};

pub enum OutputFormat {
    Text,
    Json,
}

pub struct SearchCommandOptions {
    pub format: OutputFormat,
    pub limit: usize,
    pub kind: Option<SearchKind>,
    pub page: Option<String>,
}

pub fn run(file: &Path, query: &str, options: SearchCommandOptions) -> Result<()> {
    let package = McdPackage::open_path(file)?;
    let hits = package.search(
        query,
        SearchOptions {
            limit: options.limit,
            kind: options.kind,
            page: options.page,
        },
    )?;

    match options.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&hits)?);
        }
        OutputFormat::Text => {
            for hit in hits {
                let line = hit
                    .line_start
                    .map(|line| format!(":{line}"))
                    .unwrap_or_default();
                let heading = hit
                    .heading
                    .as_deref()
                    .map(|heading| format!(" [{heading}]"))
                    .unwrap_or_default();
                println!(
                    "{}{} {} {:.3}{} {}",
                    hit.path, line, hit.kind, hit.score, heading, hit.text
                );
            }
        }
    }
    Ok(())
}
