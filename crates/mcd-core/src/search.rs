//! In-memory BM25 search over package content and metadata.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    McdPackage,
    annotations::load_manifest_annotations,
    document::{DocumentBlock, McdDocument, SourceSpan},
    markdown,
    provenance::load_manifest_provenance,
    schema::{TableColumnSchema, TableSchema},
};

const DEFAULT_LIMIT: usize = 10;
const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;

/// Search corpus kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    /// Markdown content blocks.
    Markdown,
    /// Table schema metadata.
    Schema,
    /// Manifest metadata.
    Manifest,
    /// Annotation metadata.
    Annotation,
    /// Provenance metadata.
    Provenance,
}

impl SearchKind {
    /// Parse a search kind name.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "markdown" => Some(Self::Markdown),
            "schema" => Some(Self::Schema),
            "manifest" => Some(Self::Manifest),
            "annotation" => Some(Self::Annotation),
            "provenance" => Some(Self::Provenance),
            _ => None,
        }
    }
}

impl std::fmt::Display for SearchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Markdown => "markdown",
            Self::Schema => "schema",
            Self::Manifest => "manifest",
            Self::Annotation => "annotation",
            Self::Provenance => "provenance",
        };
        f.write_str(value)
    }
}

/// Search options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    /// Maximum result count.
    pub limit: usize,
    /// Optional kind filter.
    pub kind: Option<SearchKind>,
    /// Optional package path filter.
    pub page: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            kind: None,
            page: None,
        }
    }
}

/// Structured search result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Package-internal source path.
    pub path: String,
    /// Hit kind.
    pub kind: SearchKind,
    /// Nearest Markdown heading or logical metadata title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// 1-based starting line when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,
    /// 1-based ending line when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_end: Option<usize>,
    /// BM25 relevance score.
    pub score: f64,
    /// Indexed text snippet.
    pub text: String,
}

/// Search a package with an in-memory BM25 index.
pub fn search_package(
    package: &McdPackage,
    query: &str,
    options: SearchOptions,
) -> crate::Result<Vec<SearchHit>> {
    let query_terms = unique_tokens(query);
    if query_terms.is_empty() || options.limit == 0 {
        return Ok(Vec::new());
    }

    let mut items = collect_corpus(package)?;
    items.retain(|item| {
        options.kind.is_none_or(|kind| item.kind == kind)
            && options.page.as_deref().is_none_or(|page| item.path == page)
    });
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let document_count = items.len() as f64;
    let tokenized = items
        .iter()
        .map(|item| tokenize(&item.search_text))
        .collect::<Vec<_>>();
    let average_len = tokenized
        .iter()
        .map(|tokens| tokens.len() as f64)
        .sum::<f64>()
        / document_count;

    let mut document_frequency: HashMap<String, usize> = HashMap::new();
    for tokens in &tokenized {
        let seen = tokens.iter().map(String::as_str).collect::<HashSet<_>>();
        for token in seen {
            *document_frequency.entry(token.to_owned()).or_default() += 1;
        }
    }

    let mut scored = items
        .into_iter()
        .zip(tokenized)
        .filter_map(|(item, tokens)| {
            let score = bm25_score(
                &query_terms,
                &tokens,
                &document_frequency,
                document_count,
                average_len,
            );
            (score > 0.0).then_some((item, score))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.kind.to_string().cmp(&right.kind.to_string()))
            .then_with(|| left.line_start.cmp(&right.line_start))
            .then_with(|| left.text.cmp(&right.text))
    });

    Ok(scored
        .into_iter()
        .take(options.limit)
        .map(|(item, score)| SearchHit {
            path: item.path,
            kind: item.kind,
            heading: item.heading,
            line_start: item.line_start,
            line_end: item.line_end,
            score,
            text: item.text,
        })
        .collect())
}

#[derive(Clone, Debug)]
struct CorpusItem {
    path: String,
    kind: SearchKind,
    heading: Option<String>,
    line_start: Option<usize>,
    line_end: Option<usize>,
    text: String,
    search_text: String,
}

fn collect_corpus(package: &McdPackage) -> crate::Result<Vec<CorpusItem>> {
    let manifest = package.manifest()?;
    let mut items = Vec::new();

    for path in package
        .entry_paths()
        .into_iter()
        .filter(|path| path.ends_with(".md"))
    {
        let markdown = package.read_to_string(path)?;
        let document = markdown::parse_markdown(path, &markdown)?;
        push_markdown_items(&mut items, &document);
    }

    push_manifest_items(&mut items, &manifest);
    for table in &manifest.tables {
        let schema = TableSchema::from_package(package, &table.schema)?;
        push_schema_items(&mut items, &table.id, &table.schema, &schema);
    }

    let entry_document = McdDocument::from_package(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &entry_document)?;
    for (id, annotation) in annotations {
        let value = serde_json::to_value(&annotation).unwrap_or(Value::Null);
        items.push(CorpusItem {
            path: manifest
                .annotations
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.metadata.clone())
                .unwrap_or_else(|| "manifest.json".to_owned()),
            kind: SearchKind::Annotation,
            heading: Some(id.clone()),
            line_start: None,
            line_end: None,
            text: compact_join(json_strings(&value)),
            search_text: compact_join(json_strings(&value)),
        });
    }

    if let Some(provenance) = load_manifest_provenance(package, &manifest)?
        && let Some(path) = &manifest.provenance
    {
        let value = serde_json::to_value(&provenance).unwrap_or(Value::Null);
        let text = compact_join(json_strings(&value));
        items.push(CorpusItem {
            path: path.clone(),
            kind: SearchKind::Provenance,
            heading: Some("provenance".to_owned()),
            line_start: None,
            line_end: None,
            text: text.clone(),
            search_text: text,
        });
    }

    Ok(items)
}

fn push_markdown_items(items: &mut Vec<CorpusItem>, document: &McdDocument) {
    let mut current_heading: Option<String> = None;
    for block in &document.blocks {
        match block {
            DocumentBlock::Heading { text, source, .. } => {
                current_heading = Some(text.clone());
                push_markdown_text(items, document, Some(text.clone()), *source, text.clone());
            }
            DocumentBlock::Paragraph { text, source, .. }
            | DocumentBlock::List { text, source, .. }
            | DocumentBlock::Quote { text, source, .. }
            | DocumentBlock::MathBlock { text, source, .. } => {
                push_markdown_text(
                    items,
                    document,
                    current_heading.clone(),
                    *source,
                    text.clone(),
                );
            }
            DocumentBlock::CodeBlock {
                text,
                source,
                language,
                ..
            } => {
                let display = language
                    .as_deref()
                    .map(|language| format!("{language}\n{text}"))
                    .unwrap_or_else(|| text.clone());
                push_markdown_text(items, document, current_heading.clone(), *source, display);
            }
            DocumentBlock::TableRef {
                placement, source, ..
            } => {
                let text = [
                    placement.ref_id.as_deref(),
                    Some(placement.table.as_str()),
                    placement.view.as_deref(),
                    placement.caption.as_deref(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
                push_markdown_text(items, document, current_heading.clone(), *source, text);
            }
            DocumentBlock::ImageRef {
                placement, source, ..
            } => {
                let text = [
                    placement.ref_id.as_deref(),
                    placement.asset.as_deref(),
                    placement.image.as_deref(),
                    placement.alt.as_deref(),
                    placement.caption.as_deref(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
                push_markdown_text(items, document, current_heading.clone(), *source, text);
            }
        }
    }
}

fn push_markdown_text(
    items: &mut Vec<CorpusItem>,
    document: &McdDocument,
    heading: Option<String>,
    source: Option<SourceSpan>,
    text: String,
) {
    if text.trim().is_empty() {
        return;
    }
    items.push(CorpusItem {
        path: document.source_path.clone(),
        kind: SearchKind::Markdown,
        heading,
        line_start: source.map(|source| source.start_line),
        line_end: source.map(|source| source.end_line),
        search_text: text.clone(),
        text,
    });
}

fn push_manifest_items(items: &mut Vec<CorpusItem>, manifest: &crate::Manifest) {
    let mut parts = vec![
        manifest.format.clone(),
        manifest.version.clone(),
        manifest.entrypoint.clone(),
    ];
    if let Some(title) = &manifest.title {
        parts.push(title.clone());
    }
    parts.extend(manifest.tables.iter().flat_map(|table| {
        [
            table.id.clone(),
            table.data.clone(),
            table.schema.clone(),
            table.views.keys().cloned().collect::<Vec<_>>().join(" "),
            table.views.values().cloned().collect::<Vec<_>>().join(" "),
        ]
    }));
    parts.extend(
        manifest
            .images
            .iter()
            .flat_map(|image| [image.id.clone(), image.metadata.clone()]),
    );
    parts.extend(manifest.external_data.iter().flat_map(|item| {
        [
            item.id.clone(),
            item.uri.clone(),
            item.media_type.clone(),
            item.description.clone().unwrap_or_default(),
        ]
    }));
    let text = compact_join(parts);
    if !text.is_empty() {
        items.push(CorpusItem {
            path: "manifest.json".to_owned(),
            kind: SearchKind::Manifest,
            heading: manifest
                .title
                .clone()
                .or_else(|| Some("manifest".to_owned())),
            line_start: None,
            line_end: None,
            text: text.clone(),
            search_text: text,
        });
    }
}

fn push_schema_items(
    items: &mut Vec<CorpusItem>,
    table_id: &str,
    path: &str,
    schema: &TableSchema,
) {
    let table_text = compact_join([
        table_id.to_owned(),
        schema.id.clone(),
        (!schema.primary_key.is_empty())
            .then(|| format!("primary key {}", schema.primary_key.join(" ")))
            .unwrap_or_default(),
        schema
            .foreign_keys
            .iter()
            .map(|key| {
                format!(
                    "foreign key {} references {} {}",
                    key.columns.join(" "),
                    key.references.table,
                    key.references.columns.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join(" "),
    ]);
    if !table_text.is_empty() {
        items.push(CorpusItem {
            path: path.to_owned(),
            kind: SearchKind::Schema,
            heading: Some(table_id.to_owned()),
            line_start: None,
            line_end: None,
            text: table_text.clone(),
            search_text: table_text,
        });
    }

    for column in &schema.columns {
        let text = column_text(table_id, column);
        items.push(CorpusItem {
            path: path.to_owned(),
            kind: SearchKind::Schema,
            heading: Some(format!("{table_id}.{}", column.name)),
            line_start: None,
            line_end: None,
            text: text.clone(),
            search_text: text,
        });
    }
}

fn column_text(table_id: &str, column: &TableColumnSchema) -> String {
    let mut parts = vec![
        table_id.to_owned(),
        column.name.clone(),
        column.value_type.to_string(),
    ];
    if let Some(label) = &column.label {
        parts.push(label.clone());
    }
    if let Some(unit) = &column.unit {
        if let Some(code) = &unit.code {
            parts.push(code.clone());
        }
        if let Some(label) = &unit.label {
            parts.push(label.clone());
        }
    }
    parts.extend(column.enum_values.clone());
    compact_join(parts)
}

fn bm25_score(
    query_terms: &[String],
    tokens: &[String],
    document_frequency: &HashMap<String, usize>,
    document_count: f64,
    average_len: f64,
) -> f64 {
    if tokens.is_empty() || average_len == 0.0 {
        return 0.0;
    }
    let mut term_frequency: HashMap<&str, usize> = HashMap::new();
    for token in tokens {
        *term_frequency.entry(token).or_default() += 1;
    }
    let document_len = tokens.len() as f64;
    query_terms
        .iter()
        .filter_map(|term| {
            let frequency = *term_frequency.get(term.as_str())? as f64;
            let document_frequency = *document_frequency.get(term.as_str()).unwrap_or(&0) as f64;
            let idf = (1.0
                + (document_count - document_frequency + 0.5) / (document_frequency + 0.5))
                .ln();
            let denominator =
                frequency + BM25_K1 * (1.0 - BM25_B + BM25_B * document_len / average_len);
            Some(idf * frequency * (BM25_K1 + 1.0) / denominator)
        })
        .sum()
}

fn unique_tokens(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    tokenize(text)
        .into_iter()
        .filter(|token| seen.insert(token.clone()))
        .collect()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character.to_ascii_lowercase());
        } else {
            push_token_parts(&mut tokens, &mut current);
        }
    }
    push_token_parts(&mut tokens, &mut current);
    tokens
}

fn push_token_parts(tokens: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    tokens.push(current.clone());
    if current.contains('_') {
        tokens.extend(
            current
                .split('_')
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned),
        );
    }
    current.clear();
}

fn json_strings(value: &Value) -> Vec<String> {
    let mut strings = Vec::new();
    collect_json_strings(value, &mut strings);
    strings
}

fn collect_json_strings(value: &Value, strings: &mut Vec<String>) {
    match value {
        Value::String(value) if !value.trim().is_empty() => strings.push(value.clone()),
        Value::Array(values) => {
            for value in values {
                collect_json_strings(value, strings);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                strings.push(key.clone());
                collect_json_strings(value, strings);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn compact_join(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl std::fmt::Display for crate::schema::ColumnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::Time => "time",
            Self::Enum => "enum",
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn searches_markdown_and_schema_without_rows() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{
                    "format":"MCD",
                    "version":"0.1",
                    "profile":"MCD-Core",
                    "entrypoint":"content/main.md",
                    "title":"Thermal Dossier",
                    "tables":[{"id":"powertrain","data":"tables/powertrain.csv","schema":"tables/powertrain.schema.json"}]
                }"#,
            ),
            (
                "content/main.md",
                "# Powertrain calibration specifications\n\nThe `thermal_limit_deg_c` field constrains coolant flow for V50D.\n",
            ),
            ("tables/powertrain.csv", "calibration_id,engine_family\nCAL-1,V50D\n"),
            (
                "tables/powertrain.schema.json",
                r#"{"id":"powertrain","columns":[{"name":"calibration_id","type":"string","label":"Calibration ID"},{"name":"thermal_limit_deg_c","type":"decimal","label":"Thermal Limit","unit":{"code":"deg_C","label":"deg C"}}]}"#,
            ),
        ]))
        .expect("package opens");

        let hits = search_package(
            &package,
            "thermal_limit_deg_c coolant V50D",
            SearchOptions {
                limit: 5,
                kind: None,
                page: None,
            },
        )
        .expect("search succeeds");

        assert!(hits.iter().any(|hit| {
            hit.kind == SearchKind::Markdown
                && hit.path == "content/main.md"
                && hit.line_start == Some(3)
        }));
        assert!(hits.iter().any(|hit| {
            hit.kind == SearchKind::Schema
                && hit.path == "tables/powertrain.schema.json"
                && hit.text.contains("thermal_limit_deg_c")
        }));
        assert!(!hits.iter().any(|hit| hit.text.contains("CAL-1")));
    }

    #[test]
    fn filters_kind_and_page() {
        let package = McdPackage::from_markdown("# Title\n\nA coolant paragraph.\n");
        let hits = package
            .search(
                "coolant",
                SearchOptions {
                    limit: 10,
                    kind: Some(SearchKind::Markdown),
                    page: Some("content/main.md".to_owned()),
                },
            )
            .expect("search succeeds");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_start, Some(3));
    }

    fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(*path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip").into_inner()
    }
}
