use std::path::Path;

use anyhow::Result;
use mcd_core::{McdPackage, schema::TableSchema};
use serde_json::{Value, json};

pub enum OutputFormat {
    Text,
    Json,
}

pub fn run(file: Option<&Path>, format: OutputFormat) -> Result<()> {
    let package = match file {
        Some(path) => Some(package_schema_summary(path)?),
        None => None,
    };
    let guide = guide_json(package);

    match format {
        OutputFormat::Text => print!("{}", guide_text(&guide)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&guide)?),
    }
    Ok(())
}

fn guide_json(package: Option<Value>) -> Value {
    let mut guide = json!({
        "python": {
            "import": "import mcd",
            "topLevel": [
                "mcd.open(path) -> Document",
                "mcd.query(path, sql) -> QueryResult",
                "mcd.convert_pdf(input, output, title=None) -> Document",
                "mcd.pdf_to_mcd_bytes(pdf, title=None, source_filename=None) -> bytes"
            ],
            "document": [
                "doc.validate() -> ValidationResult",
                "doc.blocks() -> list[Block]",
                "doc.table(id) -> Table",
                "doc.chart(id) -> Chart",
                "doc.image(id) -> Image",
                "doc.annotation(id) -> Annotation",
                "doc.annotations() -> list[Annotation]",
                "doc.markdown(expand_tables=False) -> str",
                "doc.query(sql) -> QueryResult",
                "doc.to_agent_context(include_tables=True, include_layout=False) -> dict"
            ],
            "queryResult": [
                "result.columns -> list[str]",
                "result.rows -> list[dict]",
                "result.row_count -> int",
                "len(result) -> int",
                "result.values() -> typed row values",
                "result.as_dict() -> dict",
                "result.to_json() -> str",
                "result.to_csv() -> str",
                "result.to_table() -> str"
            ],
            "objects": {
                "Table": [
                    "table.id",
                    "table.source",
                    "table.schema",
                    "table.rows()",
                    "table.typed_rows()",
                    "table.dataframe()",
                    "table.as_dict()"
                ],
                "TableSchema": [
                    "schema.id",
                    "schema.columns",
                    "schema.as_dict()"
                ],
                "Chart": [
                    "chart.table_id",
                    "chart.view_id",
                    "chart.placement_ref",
                    "chart.view",
                    "chart.rows()",
                    "chart.dataframe()",
                    "chart.to_markdown_table()",
                    "chart.layout()",
                    "chart.as_dict()"
                ],
                "Image": [
                    "image.id",
                    "image.asset_path",
                    "image.role",
                    "image.alt",
                    "image.caption",
                    "image.intrinsic_size",
                    "image.as_dict()"
                ],
                "Annotation": [
                    "annotation.id",
                    "annotation.kind",
                    "annotation.status",
                    "annotation.body",
                    "annotation.labels",
                    "annotation.target()",
                    "annotation.proposed_change()",
                    "annotation.as_dict()"
                ]
            }
        },
        "sql": {
            "cli": [
                "mcd query <file> \"select count(*) as rows from table_id\"",
                "mcd query <file> \"select * from table_id limit 5\" --format json",
                "mcd query <file> \"select column, max(metric) from table_id group by column\" --format csv"
            ],
            "python": [
                "doc.query(\"select count(*) as rows from table_id\")",
                "mcd.query(path, \"select * from table_id limit 5\")"
            ],
            "supported": [
                "select",
                "with",
                "where",
                "join",
                "group by",
                "order by",
                "limit",
                "count(*)",
                "min(column)",
                "max(column)",
                "avg(column)",
                "sum(column)",
                "derived expressions"
            ],
            "readOnly": true,
            "guidance": [
                "Use SQL for large tables instead of loading all rows.",
                "Use table IDs from manifest.json as SQL table names.",
                "Inspect schema columns before writing SQL against unfamiliar packages.",
                "Return table names, column names, condition values, and result values in final answers."
            ]
        },
        "docs": [
            "MCD_PYTHON_TOOL_GUIDE.md",
            "CLI_COMMANDS.md"
        ]
    });

    if let Some(package) = package {
        guide["package"] = package;
    }
    guide
}

fn package_schema_summary(path: &Path) -> Result<Value> {
    let package = McdPackage::open_path(path)?;
    let manifest = package.manifest()?;
    let tables = manifest
        .tables
        .iter()
        .map(|entry| {
            let schema = TableSchema::from_package(&package, &entry.schema)?;
            let columns = schema
                .columns
                .iter()
                .map(|column| {
                    json!({
                        "name": column.name.as_str(),
                        "type": format!("{:?}", column.value_type).to_ascii_lowercase(),
                        "label": column.label.as_deref(),
                        "nullable": column.nullable,
                        "enumValues": &column.enum_values,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "id": entry.id.as_str(),
                "data": entry.data.as_str(),
                "schema": entry.schema.as_str(),
                "views": &entry.views,
                "columns": columns,
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "file": path.display().to_string(),
        "title": manifest.title.as_deref(),
        "entrypoint": manifest.entrypoint.as_str(),
        "tables": tables,
    }))
}

fn guide_text(guide: &Value) -> String {
    let mut lines = Vec::new();
    lines.push("# MCD Python and SQL Tools".to_owned());
    lines.push(String::new());
    lines.push("Python import:".to_owned());
    lines.push("  import mcd".to_owned());
    lines.push(String::new());
    push_list(
        &mut lines,
        "Python top-level commands:",
        &guide["python"]["topLevel"],
    );
    push_list(
        &mut lines,
        "Document commands:",
        &guide["python"]["document"],
    );
    push_list(
        &mut lines,
        "QueryResult commands:",
        &guide["python"]["queryResult"],
    );
    push_list(&mut lines, "SQL CLI examples:", &guide["sql"]["cli"]);
    push_list(&mut lines, "SQL Python examples:", &guide["sql"]["python"]);
    push_list(&mut lines, "SQL supports:", &guide["sql"]["supported"]);
    push_list(&mut lines, "Agent guidance:", &guide["sql"]["guidance"]);

    if let Some(package) = guide.get("package") {
        lines.push("Package tables:".to_owned());
        lines.push(format!(
            "  file: {}",
            package["file"].as_str().unwrap_or_default()
        ));
        if let Some(title) = package["title"].as_str() {
            lines.push(format!("  title: {title}"));
        }
        lines.push(format!(
            "  entrypoint: {}",
            package["entrypoint"].as_str().unwrap_or_default()
        ));
        if let Some(tables) = package["tables"].as_array() {
            for table in tables {
                lines.push(format!(
                    "  - {} ({})",
                    table["id"].as_str().unwrap_or_default(),
                    table["data"].as_str().unwrap_or_default()
                ));
                if let Some(columns) = table["columns"].as_array() {
                    for column in columns {
                        let nullable = if column["nullable"].as_bool().unwrap_or(false) {
                            ", nullable"
                        } else {
                            ""
                        };
                        lines.push(format!(
                            "      {}: {}{}",
                            column["name"].as_str().unwrap_or_default(),
                            column["type"].as_str().unwrap_or_default(),
                            nullable
                        ));
                    }
                }
            }
        }
        lines.push(String::new());
    }

    lines.push("Docs:".to_owned());
    lines.push("  MCD_PYTHON_TOOL_GUIDE.md".to_owned());
    lines.push("  CLI_COMMANDS.md".to_owned());
    lines.join("\n") + "\n"
}

fn push_list(lines: &mut Vec<String>, title: &str, values: &Value) {
    lines.push(title.to_owned());
    if let Some(items) = values.as_array() {
        for item in items {
            lines.push(format!("  - {}", item.as_str().unwrap_or_default()));
        }
    }
    lines.push(String::new());
}
