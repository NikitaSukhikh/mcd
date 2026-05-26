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
                "doc.external_data() -> list[dict]",
                "doc.provenance() -> dict | None",
                "doc.relationships() -> list[dict]",
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
                    "schema.primary_key",
                    "schema.foreign_keys",
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
                        "unit": &column.unit,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "id": entry.id.as_str(),
                "data": entry.data.as_str(),
                "schema": entry.schema.as_str(),
                "primaryKey": &schema.primary_key,
                "foreignKeys": &schema.foreign_keys,
                "views": &entry.views,
                "columns": columns,
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "file": path.display().to_string(),
        "title": manifest.title.as_deref(),
        "entrypoint": manifest.entrypoint.as_str(),
        "externalData": &manifest.external_data,
        "provenance": manifest.provenance.as_deref(),
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
                if let Some(primary_key) = table["primaryKey"].as_array()
                    && !primary_key.is_empty()
                {
                    let columns = primary_key
                        .iter()
                        .filter_map(|value| value.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("      primary key: {columns}"));
                }
                if let Some(foreign_keys) = table["foreignKeys"].as_array() {
                    for foreign_key in foreign_keys {
                        let columns = foreign_key["columns"]
                            .as_array()
                            .map(|columns| {
                                columns
                                    .iter()
                                    .filter_map(|value| value.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        let target_table = foreign_key["references"]["table"]
                            .as_str()
                            .unwrap_or_default();
                        let target_columns = foreign_key["references"]["columns"]
                            .as_array()
                            .map(|columns| {
                                columns
                                    .iter()
                                    .filter_map(|value| value.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        lines.push(format!(
                            "      foreign key: ({columns}) -> {target_table}({target_columns})"
                        ));
                    }
                }
                if let Some(columns) = table["columns"].as_array() {
                    for column in columns {
                        let nullable = if column["nullable"].as_bool().unwrap_or(false) {
                            ", nullable"
                        } else {
                            ""
                        };
                        let unit = column["unit"]
                            .as_object()
                            .and_then(|unit| {
                                unit.get("label")
                                    .or_else(|| unit.get("code"))
                                    .and_then(|value| value.as_str())
                            })
                            .map(|unit| format!(", unit {unit}"))
                            .unwrap_or_default();
                        lines.push(format!(
                            "      {}: {}{}{}",
                            column["name"].as_str().unwrap_or_default(),
                            column["type"].as_str().unwrap_or_default(),
                            nullable,
                            unit
                        ));
                    }
                }
            }
        }
        if let Some(external_data) = package["externalData"].as_array()
            && !external_data.is_empty()
        {
            lines.push("Package external data:".to_owned());
            for item in external_data {
                lines.push(format!(
                    "  - {} ({}, {})",
                    item["id"].as_str().unwrap_or_default(),
                    item["mediaType"].as_str().unwrap_or_default(),
                    item["uri"].as_str().unwrap_or_default()
                ));
            }
        }
        if let Some(provenance) = package["provenance"].as_str() {
            lines.push(format!("Package provenance: {provenance}"));
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
