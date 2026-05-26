//! Model Context Protocol server implementation for MCD packages.

use std::{
    fs,
    fs::File,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use mcd_core::{
    McdError, McdPackage,
    annotations::{AnnotationMetadata, AnnotationTarget},
    document::SourceSpan,
    export::{
        annotation_export, chart_export, expanded_markdown_export, external_data_export,
        image_export, original_markdown_export, provenance_export, schema_summary_export,
        table_export,
    },
    package::{MCD_MIMETYPE, validate_internal_path},
    pdf::{PdfConversionOptions, pdf_to_mcd_bytes},
    validate::validate_package,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const PROTOCOL_VERSION: &str = "2025-06-18";

/// Run the server over newline-delimited JSON-RPC on stdin/stdout.
pub fn run_stdio() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    run_stdio_with(stdin.lock(), &mut stdout)
}

/// Run the stdio server with caller-provided streams.
pub fn run_stdio_with<R, W>(reader: R, writer: &mut W) -> Result<()>
where
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_json_rpc_line(&line) {
            serde_json::to_writer(&mut *writer, &response)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}

fn handle_json_rpc_line(line: &str) -> Option<Value> {
    let request = match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => request,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                -32700,
                format!("parse error: {err}"),
            ));
        }
    };

    handle_request(request)
}

fn handle_request(request: JsonRpcRequest) -> Option<Value> {
    let id = request.id.clone();
    let is_notification = id.is_none();
    let result = match request.method.as_str() {
        "initialize" => Ok(initialize_result()),
        "notifications/initialized" => return None,
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool_request(request.params),
        method => Err(JsonRpcError {
            code: -32601,
            message: format!("method not found: {method}"),
        }),
    };

    if is_notification {
        return None;
    }

    let id = id.unwrap_or(Value::Null);
    Some(match result {
        Ok(result) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        }),
        Err(err) => error_response(id, err.code, err.message),
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "mcd-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

fn call_tool_request(params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| invalid_params("missing tools/call params"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("tools/call params.name must be a string"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    if !arguments.is_object() {
        return Err(invalid_params(
            "tools/call params.arguments must be an object",
        ));
    }

    Ok(match call_tool(name, arguments) {
        Ok(value) => tool_success(value),
        Err(err) => tool_error(err.to_string()),
    })
}

fn invalid_params(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: message.into(),
    }
}

fn call_tool(name: &str, arguments: Value) -> Result<Value> {
    match name {
        "mcd_validate" => mcd_validate(arguments),
        "mcd_inspect" => mcd_inspect(arguments),
        "mcd_agent_context" => mcd_agent_context(arguments),
        "mcd_markdown" => mcd_markdown(arguments),
        "mcd_query" => mcd_query(arguments),
        "mcd_queries" => mcd_queries(arguments),
        "mcd_table" => mcd_table(arguments),
        "mcd_schemas" => mcd_schemas(arguments),
        "mcd_chart" => mcd_chart(arguments),
        "mcd_images" => mcd_images(arguments),
        "mcd_annotations" => mcd_annotations(arguments),
        "mcd_relationships" => mcd_relationships(arguments),
        "mcd_external_data" => mcd_external_data(arguments),
        "mcd_provenance" => mcd_provenance(arguments),
        "mcd_render" => mcd_render(arguments),
        "mcd_pack" => mcd_pack(arguments),
        "mcd_unpack" => mcd_unpack(arguments),
        "mcd_init" => mcd_init(arguments),
        "mcd_add_annotation" => mcd_add_annotation(arguments),
        "mcd_convert_pdf" => mcd_convert_pdf(arguments),
        other => bail!("unknown MCD MCP tool: {other}"),
    }
}

fn tool_success(value: Value) -> Value {
    let text = match &value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
    };
    json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "structuredContent": value,
        "isError": false
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "isError": true
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolSpec {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

fn tools() -> Vec<ToolSpec> {
    vec![
        tool(
            "mcd_validate",
            "Validate an MCD package and return structured diagnostics.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_inspect",
            "Return a compact package manifest and entry summary.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_agent_context",
            "Return an agent-oriented package context.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    boolean_prop("includeTables", "Include table rows in the context.", false),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_markdown",
            "Return original or expanded package Markdown.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    boolean_prop("expandTables", "Expand MCD table directives.", false),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_query",
            "Run read-only SQL against package tables and MCD metadata tables.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    required_string("sql", "Read-only SELECT or WITH query."),
                    enum_prop(
                        "format",
                        &["json", "csv", "table"],
                        "Result format.",
                        "json",
                    ),
                ],
                &["path", "sql"],
            ),
        ),
        tool(
            "mcd_queries",
            "Run multiple read-only SQL queries against one package-loaded SQLite database.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    array_prop("queries", "Read-only SELECT or WITH queries."),
                ],
                &["path", "queries"],
            ),
        ),
        tool(
            "mcd_table",
            "Return all tables or one table from an MCD package.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    string_prop("tableId", "Optional table id to select."),
                    integer_prop("maxRows", "Optional maximum rows per returned table."),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_schemas",
            "Return table schemas, keys, relationships, and units.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_chart",
            "Return chart metadata and source rows.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    string_prop(
                        "chartId",
                        "Optional chart block id, view id, or placement ref.",
                    ),
                    integer_prop("maxRows", "Optional maximum rows per returned chart."),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_images",
            "Return package image metadata.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    string_prop("imageId", "Optional image id or asset path."),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_annotations",
            "Return package annotations, optionally filtered by id, page, or line.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    string_prop("annotationId", "Optional annotation id."),
                    string_prop("page", "Optional internal package page path."),
                    integer_prop("line", "Optional one-based line number."),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_relationships",
            "Return declared table relationships.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_external_data",
            "Return manifest-declared external data references.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_provenance",
            "Return package provenance metadata.",
            object_schema(
                &[required_string("path", "Path to the .mcd package.")],
                &["path"],
            ),
        ),
        tool(
            "mcd_render",
            "Render an MCD package to HTML or expanded Markdown, returning content or writing to output.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    enum_prop("target", &["html", "markdown"], "Render target.", "html"),
                    string_prop("output", "Optional output file or HTML project directory."),
                ],
                &["path"],
            ),
        ),
        tool(
            "mcd_pack",
            "Pack an unpacked MCD directory into a .mcd package.",
            object_schema(
                &[
                    required_string("directory", "Unpacked MCD directory."),
                    required_string("output", "Output .mcd package path."),
                ],
                &["directory", "output"],
            ),
        ),
        tool(
            "mcd_unpack",
            "Unpack an MCD package into a directory without overwriting files.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package."),
                    required_string("output", "Output directory."),
                ],
                &["path", "output"],
            ),
        ),
        tool(
            "mcd_init",
            "Initialize a minimal unpacked MCD directory.",
            object_schema(
                &[required_string("directory", "Directory to initialize.")],
                &["directory"],
            ),
        ),
        tool(
            "mcd_convert_pdf",
            "Convert a PDF file to an MCD package.",
            object_schema(
                &[
                    required_string("input", "Input PDF path."),
                    required_string("output", "Output .mcd package path."),
                    string_prop("title", "Optional document title."),
                ],
                &["input", "output"],
            ),
        ),
        tool(
            "mcd_add_annotation",
            "Add a plain-text annotation to an MCD package.",
            object_schema(
                &[
                    required_string("path", "Path to the .mcd package to update."),
                    required_string("text", "Annotation body text."),
                    required_string("page", "Internal package path/page to target."),
                    integer_prop("line", "Optional one-based source line."),
                    string_prop("id", "Optional stable annotation id."),
                ],
                &["path", "text", "page"],
            ),
        ),
    ]
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> ToolSpec {
    ToolSpec {
        name,
        description,
        input_schema,
    }
}

fn object_schema(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let mut property_map = Map::new();
    for (name, schema) in properties {
        property_map.insert((*name).to_owned(), schema.clone());
    }
    json!({
        "type": "object",
        "properties": property_map,
        "required": required,
        "additionalProperties": false
    })
}

fn required_string<'a>(name: &'a str, description: &str) -> (&'a str, Value) {
    (
        name,
        json!({ "type": "string", "description": description }),
    )
}

fn string_prop<'a>(name: &'a str, description: &str) -> (&'a str, Value) {
    required_string(name, description)
}

fn boolean_prop<'a>(name: &'a str, description: &str, default: bool) -> (&'a str, Value) {
    (
        name,
        json!({ "type": "boolean", "description": description, "default": default }),
    )
}

fn integer_prop<'a>(name: &'a str, description: &str) -> (&'a str, Value) {
    (
        name,
        json!({ "type": "integer", "minimum": 0, "description": description }),
    )
}

fn enum_prop<'a>(
    name: &'a str,
    values: &[&str],
    description: &str,
    default: &str,
) -> (&'a str, Value) {
    (
        name,
        json!({
            "type": "string",
            "enum": values,
            "description": description,
            "default": default
        }),
    )
}

fn array_prop<'a>(name: &'a str, description: &str) -> (&'a str, Value) {
    (
        name,
        json!({
            "type": "array",
            "items": { "type": "string" },
            "minItems": 1,
            "description": description
        }),
    )
}

fn mcd_validate(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = match McdPackage::open_path(&path) {
        Ok(package) => package,
        Err(err) => return Ok(validation_error(err)),
    };
    match validate_package(&package) {
        Ok(result) => Ok(serde_json::to_value(result)?),
        Err(err) => Ok(validation_error(err)),
    }
}

fn validation_error(err: McdError) -> Value {
    if let Some(diagnostic) = err.diagnostic() {
        json!({
            "valid": false,
            "diagnostics": [diagnostic],
        })
    } else {
        json!({
            "valid": false,
            "diagnostics": [{
                "level": "error",
                "code": "mcd.error",
                "message": err.to_string()
            }],
        })
    }
}

fn mcd_inspect(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = McdPackage::open_path(&path)?;
    let manifest = package.manifest()?;
    Ok(json!({
        "format": manifest.format,
        "version": manifest.version,
        "profile": manifest.profile,
        "entrypoint": manifest.entrypoint,
        "tables": manifest.tables.len(),
        "annotations": manifest.annotations.len(),
        "externalData": manifest.external_data.len(),
        "provenance": manifest.provenance.is_some(),
        "entries": package.entry_paths().len(),
    }))
}

fn mcd_agent_context(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let include_tables = bool_arg(&arguments, "includeTables", false)?;
    let package = McdPackage::open_path(&path)?;
    let mut value = serde_json::to_value(mcd_core::export::agent_context_export(&package)?)?;
    if !include_tables && let Some(object) = value.as_object_mut() {
        object.remove("tables");
    }
    Ok(value)
}

fn mcd_markdown(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let expand_tables = bool_arg(&arguments, "expandTables", false)?;
    let package = McdPackage::open_path(&path)?;
    let markdown = if expand_tables {
        expanded_markdown_export(&package)?
    } else {
        original_markdown_export(&package)?
    };
    Ok(json!({ "markdown": markdown }))
}

fn mcd_query(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let sql = required_string_arg(&arguments, "sql")?;
    let format = string_arg(&arguments, "format")?.unwrap_or_else(|| "json".to_owned());
    let package = McdPackage::open_path(&path)?;
    let result = mcd_query::query_package(&package, &sql)?;
    match format.as_str() {
        "json" => Ok(result.as_json()),
        "csv" => Ok(json!({ "csv": result.to_csv() })),
        "table" => Ok(json!({ "table": result.to_table() })),
        _ => bail!("format must be one of: json, csv, table"),
    }
}

fn mcd_queries(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let queries = required_string_array_arg(&arguments, "queries")?;
    let package = McdPackage::open_path(&path)?;
    let results = mcd_query::query_package_many(&package, &queries)?;
    let queries = queries
        .into_iter()
        .zip(results)
        .map(|(sql, result)| {
            json!({
                "sql": sql,
                "result": result.as_json(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "queryCount": queries.len(),
        "queries": queries,
    }))
}

fn mcd_table(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let table_id = string_arg(&arguments, "tableId")?;
    let max_rows = optional_usize_arg(&arguments, "maxRows")?;
    let package = McdPackage::open_path(&path)?;
    let mut export = serde_json::to_value(table_export(&package)?)?;
    if let Some(table_id) = table_id {
        retain_by_id(&mut export, "tables", "id", &table_id)?;
    }
    limit_rows(&mut export, "tables", max_rows);
    Ok(export)
}

fn mcd_schemas(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = McdPackage::open_path(&path)?;
    Ok(serde_json::to_value(schema_summary_export(&package)?)?)
}

fn mcd_chart(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let chart_id = string_arg(&arguments, "chartId")?;
    let max_rows = optional_usize_arg(&arguments, "maxRows")?;
    let package = McdPackage::open_path(&path)?;
    let mut export = serde_json::to_value(chart_export(&package)?)?;
    if let Some(chart_id) = chart_id {
        retain_chart(&mut export, &chart_id)?;
    }
    limit_rows(&mut export, "charts", max_rows);
    Ok(export)
}

fn mcd_images(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let image_id = string_arg(&arguments, "imageId")?;
    let package = McdPackage::open_path(&path)?;
    let mut export = serde_json::to_value(image_export(&package)?)?;
    if let Some(image_id) = image_id {
        retain_image(&mut export, &image_id)?;
    }
    Ok(export)
}

fn mcd_annotations(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let annotation_id = string_arg(&arguments, "annotationId")?;
    let page = string_arg(&arguments, "page")?;
    let line = optional_usize_arg(&arguments, "line")?;
    if line == Some(0) {
        bail!("line must be 1 or greater");
    }
    let package = McdPackage::open_path(&path)?;
    let mut export = annotation_export(&package)?;
    export.annotations.retain(|annotation| {
        annotation_id
            .as_ref()
            .is_none_or(|id| annotation.id.as_str() == id)
            && annotation_matches(annotation, page.as_deref(), line)
    });
    Ok(serde_json::to_value(export)?)
}

fn mcd_relationships(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = McdPackage::open_path(&path)?;
    let tables = table_export(&package)?.tables;
    let mut relationships = Vec::new();
    for table in tables {
        for foreign_key in table.schema.foreign_keys {
            relationships.push(json!({
                "tableId": table.id,
                "columns": foreign_key.columns,
                "references": foreign_key.references,
            }));
        }
    }
    Ok(json!({
        "relationships": relationships,
        "count": relationships.len(),
    }))
}

fn mcd_external_data(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = McdPackage::open_path(&path)?;
    Ok(serde_json::to_value(external_data_export(&package)?)?)
}

fn mcd_provenance(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let package = McdPackage::open_path(&path)?;
    Ok(serde_json::to_value(provenance_export(&package)?)?)
}

fn mcd_render(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let target = string_arg(&arguments, "target")?.unwrap_or_else(|| "html".to_owned());
    let output = optional_path(&arguments, "output")?;
    let package = McdPackage::open_path(&path)?;

    match (target.as_str(), output) {
        ("html", Some(output)) if should_write_html_project(&output) => {
            let rendered = mcd_render::render_html_project(&package)?;
            fs::create_dir_all(&output).with_context(|| format!("create {}", output.display()))?;
            fs::write(output.join("index.html"), rendered.index_html)?;
            fs::write(output.join("styles.css"), rendered.styles_css)?;
            for asset in rendered.assets {
                let asset_path = output.join(asset.path);
                if let Some(parent) = asset_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(asset_path, asset.bytes)?;
            }
            Ok(json!({ "output": output, "target": "html-project" }))
        }
        ("html", Some(output)) => {
            fs::write(&output, mcd_render::render_html(&package)?)?;
            Ok(json!({ "output": output, "target": "html" }))
        }
        ("markdown", Some(output)) => {
            fs::write(&output, expanded_markdown_export(&package)?)?;
            Ok(json!({ "output": output, "target": "markdown" }))
        }
        ("html", None) => Ok(json!({ "html": mcd_render::render_html(&package)? })),
        ("markdown", None) => Ok(json!({ "markdown": expanded_markdown_export(&package)? })),
        _ => bail!("target must be one of: html, markdown"),
    }
}

fn mcd_pack(arguments: Value) -> Result<Value> {
    let directory = required_path(&arguments, "directory")?;
    let output = required_output_path(&arguments, "output")?;
    pack_directory(&directory, &output)?;
    Ok(json!({ "output": output }))
}

fn mcd_unpack(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let output = required_output_path(&arguments, "output")?;
    unpack_package(&path, &output)?;
    Ok(json!({ "output": output }))
}

fn mcd_init(arguments: Value) -> Result<Value> {
    let directory = required_output_path(&arguments, "directory")?;
    let content_dir = directory.join("content");
    fs::create_dir_all(&content_dir)?;
    fs::write(directory.join("mimetype"), "application/vnd.mcd+zip\n")?;
    fs::write(
        directory.join("manifest.json"),
        r#"{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md"
}
"#,
    )?;
    fs::write(content_dir.join("main.md"), "# Untitled\n")?;
    Ok(json!({ "directory": directory }))
}

fn mcd_convert_pdf(arguments: Value) -> Result<Value> {
    let input = required_path(&arguments, "input")?;
    let output = required_output_path(&arguments, "output")?;
    let title = string_arg(&arguments, "title")?;
    let pdf = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
    let mcd = pdf_to_mcd_bytes(
        &pdf,
        PdfConversionOptions {
            title,
            source_filename: input
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned),
        },
    )?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, mcd).with_context(|| format!("write {}", output.display()))?;
    Ok(json!({ "output": output }))
}

fn mcd_add_annotation(arguments: Value) -> Result<Value> {
    let path = required_path(&arguments, "path")?;
    let text = required_string_arg(&arguments, "text")?;
    let page = required_string_arg(&arguments, "page")?;
    let line = optional_usize_arg(&arguments, "line")?;
    let id = string_arg(&arguments, "id")?;

    if text.trim().is_empty() {
        bail!("annotation text cannot be empty");
    }
    validate_internal_path(&page)?;
    if line == Some(0) {
        bail!("annotation line must be 1 or greater");
    }

    let package = McdPackage::open_path(&path)?;
    if !package.contains(&page) {
        bail!("annotation page is not present in package: {page}");
    }

    let mut manifest = manifest_json(&package)?;
    let annotation_id = match id {
        Some(id) => validate_annotation_id(&id)?,
        None => next_annotation_id(&manifest),
    };
    let metadata_path = format!("annotations/{annotation_id}.annotation.json");
    if package.contains(&metadata_path) {
        bail!("annotation metadata already exists: {metadata_path}");
    }

    append_manifest_annotation(&mut manifest, &annotation_id, &metadata_path)?;
    let annotation = annotation_json(&annotation_id, &text, &page, line);

    let mut entries = package
        .entry_paths()
        .into_iter()
        .filter(|entry| *entry != "manifest.json")
        .map(|entry| Ok((entry.to_owned(), package.read(entry)?.to_vec())))
        .collect::<std::result::Result<Vec<_>, mcd_core::McdError>>()?;
    entries.push(("manifest.json".to_owned(), serde_json::to_vec_pretty(&manifest)?));
    entries.push((
        metadata_path.clone(),
        serde_json::to_vec_pretty(&annotation)?,
    ));

    write_package(&path, entries)?;
    validate_package(&McdPackage::open_path(&path)?)?;
    Ok(json!({
        "id": annotation_id,
        "metadata": metadata_path,
        "path": path,
    }))
}

fn required_path(arguments: &Value, name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required_string_arg(arguments, name)?);
    if !path.exists() {
        bail!("{name} does not exist: {}", path.display());
    }
    Ok(path)
}

fn required_output_path(arguments: &Value, name: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(required_string_arg(arguments, name)?))
}

fn optional_path(arguments: &Value, name: &str) -> Result<Option<PathBuf>> {
    Ok(string_arg(arguments, name)?.map(PathBuf::from))
}

fn required_string_arg(arguments: &Value, name: &str) -> Result<String> {
    string_arg(arguments, name)?.ok_or_else(|| anyhow!("missing required argument: {name}"))
}

fn string_arg(arguments: &Value, name: &str) -> Result<Option<String>> {
    match arguments.get(name) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => bail!("{name} must be a string"),
    }
}

fn bool_arg(arguments: &Value, name: &str, default: bool) -> Result<bool> {
    match arguments.get(name) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Null) | None => Ok(default),
        Some(_) => bail!("{name} must be a boolean"),
    }
}

fn required_string_array_arg(arguments: &Value, name: &str) -> Result<Vec<String>> {
    let values = arguments
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("{name} must be an array of strings"))?;
    if values.is_empty() {
        bail!("{name} must contain at least one query");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow!("{name} must be an array of strings"))
        })
        .collect()
}

fn optional_usize_arg(arguments: &Value, name: &str) -> Result<Option<usize>> {
    match arguments.get(name) {
        Some(Value::Number(value)) => value
            .as_u64()
            .map(|value| Some(value as usize))
            .ok_or_else(|| anyhow!("{name} must be a non-negative integer")),
        Some(Value::Null) | None => Ok(None),
        Some(_) => bail!("{name} must be a non-negative integer"),
    }
}

fn retain_by_id(export: &mut Value, collection: &str, field: &str, id: &str) -> Result<()> {
    let items = export
        .get_mut(collection)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("missing collection: {collection}"))?;
    items.retain(|item| item.get(field).and_then(Value::as_str) == Some(id));
    if items.is_empty() {
        bail!("unknown {collection} id: {id}");
    }
    Ok(())
}

fn retain_chart(export: &mut Value, id: &str) -> Result<()> {
    let charts = export
        .get_mut("charts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("missing collection: charts"))?;
    charts.retain(|chart| {
        chart.get("blockId").and_then(Value::as_str) == Some(id)
            || chart.get("viewId").and_then(Value::as_str) == Some(id)
            || chart.get("placementRef").and_then(Value::as_str) == Some(id)
    });
    if charts.is_empty() {
        bail!("unknown chart id: {id}");
    }
    Ok(())
}

fn retain_image(export: &mut Value, id: &str) -> Result<()> {
    let images = export
        .get_mut("images")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("missing collection: images"))?;
    images.retain(|image| {
        image.get("id").and_then(Value::as_str) == Some(id)
            || image.get("asset").and_then(Value::as_str) == Some(id)
            || image
                .get("asset")
                .and_then(Value::as_str)
                .and_then(|asset| asset.strip_prefix("assets/"))
                == Some(id)
    });
    if images.is_empty() {
        bail!("unknown image id: {id}");
    }
    Ok(())
}

fn limit_rows(export: &mut Value, collection: &str, max_rows: Option<usize>) {
    let Some(max_rows) = max_rows else {
        return;
    };
    let Some(items) = export.get_mut(collection).and_then(Value::as_array_mut) else {
        return;
    };
    for item in items {
        let Some(rows) = item.get_mut("rows").and_then(Value::as_array_mut) else {
            continue;
        };
        let row_count = rows.len();
        rows.truncate(max_rows);
        let returned_row_count = rows.len();
        if let Some(object) = item.as_object_mut() {
            object.insert("rowCount".to_owned(), json!(row_count));
            object.insert("returnedRowCount".to_owned(), json!(returned_row_count));
        }
    }
}

fn annotation_matches(
    annotation: &AnnotationMetadata,
    page: Option<&str>,
    line: Option<usize>,
) -> bool {
    if page.is_none() && line.is_none() {
        return true;
    }
    let AnnotationTarget::Path { path, source } = &annotation.target else {
        return false;
    };
    if let Some(page) = page
        && path != page
    {
        return false;
    }
    if let Some(line) = line {
        let Some(source) = source else {
            return false;
        };
        return line >= source.start_line && line <= source.end_line;
    }
    true
}

fn should_write_html_project(output: &Path) -> bool {
    output.is_dir() || output.extension().is_none()
}

fn pack_directory(directory: &Path, output: &Path) -> Result<()> {
    if !directory.is_dir() {
        bail!("pack source must be a directory: {}", directory.display());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut files = collect_files(directory)?;
    files.sort();

    let output_file =
        File::create(output).with_context(|| format!("create {}", output.display()))?;
    let mut writer = ZipWriter::new(output_file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mimetype = directory.join("mimetype");
    writer.start_file("mimetype", stored)?;
    if mimetype.is_file() {
        let mut input = File::open(&mimetype)?;
        io::copy(&mut input, &mut writer)?;
        files.retain(|path| path != &mimetype);
    } else {
        writer.write_all(MCD_MIMETYPE.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    for path in files {
        let internal_path = internal_path(directory, &path)?;
        writer.start_file(&internal_path, deflated)?;
        let mut input = File::open(&path)?;
        io::copy(&mut input, &mut writer)?;
    }

    writer.finish()?;
    Ok(())
}

fn unpack_package(path: &Path, output: &Path) -> Result<()> {
    if output.exists() && !output.is_dir() {
        bail!("unpack output must be a directory: {}", output.display());
    }
    let package = McdPackage::open_path(path)?;
    fs::create_dir_all(output).with_context(|| format!("create {}", output.display()))?;
    for entry in package.entry_paths() {
        let target = output_path(output, entry);
        if target.exists() {
            bail!("refusing to overwrite existing file: {}", target.display());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, package.read(entry)?)?;
    }
    Ok(())
}

fn collect_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(directory, &mut files)?;
    Ok(files)
}

fn collect_files_inner(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_files_inner(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn internal_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => {
                parts.push(part.to_string_lossy().to_string());
            }
            _ => bail!("unsafe path in package source: {}", path.display()),
        }
    }
    let internal = parts.join("/");
    validate_internal_path(&internal)?;
    Ok(internal)
}

fn output_path(output: &Path, entry: &str) -> PathBuf {
    entry
        .split('/')
        .fold(output.to_path_buf(), |path, component| path.join(component))
}

fn manifest_json(package: &McdPackage) -> Result<Value> {
    let bytes = package.read("manifest.json")?;
    let manifest = serde_json::from_slice::<Value>(bytes)?;
    if !manifest.is_object() {
        bail!("manifest.json must be a JSON object");
    }
    Ok(manifest)
}

fn validate_annotation_id(id: &str) -> Result<String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        || !id.as_bytes()[0].is_ascii_alphanumeric()
    {
        bail!("annotation id must match [A-Za-z0-9][A-Za-z0-9_.-]*; got '{id}'");
    }
    Ok(id.to_owned())
}

fn next_annotation_id(manifest: &Value) -> String {
    let existing = manifest
        .get("annotations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();

    for index in 1.. {
        let id = format!("annotation-{index:04}");
        if !existing.contains(id.as_str()) {
            return id;
        }
    }
    unreachable!("unbounded annotation id search should always return")
}

fn append_manifest_annotation(manifest: &mut Value, id: &str, metadata_path: &str) -> Result<()> {
    let annotations = manifest
        .as_object_mut()
        .expect("manifest object checked by caller")
        .entry("annotations")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(annotations) = annotations.as_array_mut() else {
        bail!("manifest annotations field must be an array");
    };
    if annotations
        .iter()
        .any(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
    {
        bail!("annotation id already exists in manifest: {id}");
    }
    annotations.push(json!({
        "id": id,
        "metadata": metadata_path
    }));
    Ok(())
}

fn annotation_json(id: &str, text: &str, page: &str, line: Option<usize>) -> Value {
    let target = match line {
        Some(line) => json!({
            "type": "path",
            "path": page,
            "source": source_span(line)
        }),
        None => json!({
            "type": "path",
            "path": page
        }),
    };

    json!({
        "id": id,
        "target": target,
        "kind": "comment",
        "status": "open",
        "body": text
    })
}

fn source_span(line: usize) -> SourceSpan {
    SourceSpan {
        start_line: line,
        start_column: 1,
        end_line: line,
        end_column: 1,
    }
}

fn write_package(file: &Path, mut entries: Vec<(String, Vec<u8>)>) -> Result<()> {
    entries.sort_by(|left, right| entry_sort_key(&left.0).cmp(&entry_sort_key(&right.0)));

    let temp_path = temp_output_path(file);
    let output_file =
        File::create(&temp_path).with_context(|| format!("create {}", temp_path.display()))?;
    let mut writer = ZipWriter::new(output_file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut wrote_mimetype = false;
    for (path, bytes) in entries {
        validate_internal_path(&path)?;
        let options = if path == "mimetype" {
            wrote_mimetype = true;
            stored
        } else {
            deflated
        };
        writer.start_file(&path, options)?;
        writer.write_all(&bytes)?;
    }

    if !wrote_mimetype {
        writer.start_file("mimetype", stored)?;
        writer.write_all(MCD_MIMETYPE.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    writer.finish()?;
    fs::copy(&temp_path, file).with_context(|| {
        format!(
            "replace {} with updated package {}",
            file.display(),
            temp_path.display()
        )
    })?;
    fs::remove_file(&temp_path).ok();
    Ok(())
}

fn entry_sort_key(path: &str) -> (u8, &str) {
    match path {
        "mimetype" => (0, path),
        "manifest.json" => (1, path),
        _ => (2, path),
    }
}

fn temp_output_path(file: &Path) -> PathBuf {
    let mut path = file.to_path_buf();
    let extension = file
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("tmp");
    path.set_extension(format!("{extension}.tmp"));
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_mcd_tools() {
        let response =
            handle_json_rpc_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#)
                .expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools");
        assert!(tools.iter().any(|tool| tool["name"] == "mcd_validate"));
        assert!(tools.iter().any(|tool| tool["name"] == "mcd_query"));
        assert!(tools.iter().any(|tool| tool["name"] == "mcd_queries"));
        assert!(tools.iter().any(|tool| tool["name"] == "mcd_pack"));
    }

    #[test]
    fn initializes_server() {
        let response =
            handle_json_rpc_line(r#"{"jsonrpc":"2.0","id":"init","method":"initialize"}"#)
                .expect("response");
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "mcd-mcp");
    }

    #[test]
    fn calls_validate_and_query_tools_against_example_package() {
        let package = example_package("revenue-report");
        let validate = tool_call(
            "mcd_validate",
            json!({
                "path": package,
            }),
        );
        assert_eq!(validate["result"]["structuredContent"]["valid"], true);

        let query = tool_call(
            "mcd_query",
            json!({
                "path": package,
                "sql": "select count(*) as rows from revenue",
            }),
        );
        assert_eq!(
            query["result"]["structuredContent"]["rows"],
            json!([{ "rows": 4 }])
        );
    }

    #[test]
    fn tool_failures_are_returned_as_mcp_tool_errors() {
        let response = tool_call(
            "mcd_query",
            json!({
                "path": example_package("revenue-report"),
                "sql": "delete from revenue",
            }),
        );
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("error text")
                .contains("query must be a SELECT statement")
        );
    }

    #[test]
    fn calls_batch_query_tool_against_example_package() {
        let response = tool_call(
            "mcd_queries",
            json!({
                "path": example_package("revenue-report"),
                "queries": [
                    "select count(*) as rows from revenue",
                    "select quarter from revenue order by revenue_gbp desc limit 1"
                ],
            }),
        );
        assert_eq!(response["result"]["structuredContent"]["queryCount"], 2);
        assert_eq!(
            response["result"]["structuredContent"]["queries"][0]["result"]["rows"],
            json!([{ "rows": 4 }])
        );
        assert_eq!(
            response["result"]["structuredContent"]["queries"][1]["result"]["rows"],
            json!([{ "quarter": "Q4" }])
        );
    }

    fn tool_call(name: &str, arguments: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
            }
        });
        handle_json_rpc_line(&request.to_string()).expect("response")
    }

    fn example_package(name: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join(name)
            .join(format!("{name}.mcd"))
            .display()
            .to_string()
    }
}
