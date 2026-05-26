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

pub fn run_batch(file: &Path, queries: &[String]) -> Result<()> {
    let results = mcd_query::query_path_many(file, queries)?;
    let payload = serde_json::json!({
        "queryCount": results.len(),
        "queries": results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                serde_json::json!({
                    "index": index,
                    "sql": queries[index],
                    "result": result.as_json(),
                })
            })
            .collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
