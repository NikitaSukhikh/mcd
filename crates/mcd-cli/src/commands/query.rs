use std::path::Path;

use anyhow::Result;

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

pub fn run(file: &Path, sql: &str, format: OutputFormat) -> Result<()> {
    let result = mcd_query::query_path(file, sql)?;
    let output = match format {
        OutputFormat::Table => result.to_table(),
        OutputFormat::Json => result.to_json_pretty()? + "\n",
        OutputFormat::Csv => result.to_csv(),
    };
    print!("{output}");
    Ok(())
}
