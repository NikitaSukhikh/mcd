//! Export APIs.

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use crate::{
    Manifest, McdPackage,
    annotations::{
        AnnotationMetadata, AnnotationTarget, load_manifest_annotations,
        validate_annotation_markers,
    },
    directives::{ImagePlacement, TableDisplay, TablePlacement},
    document::{AnnotationRef, DocumentBlock, McdDocument, SourceSpan},
    errors::{Diagnostic, McdError},
    images::{ImageMetadata, ImageRole},
    schema::{ColumnType, TableColumnSchema},
    table_view::{ChartEncoding, TableView, ViewColumn},
    tables::{DataTable, TableRow, TypedValue},
};

/// Canonical JSON export for an MCD package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonExport {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Parsed Markdown document.
    pub document: McdDocument,
    /// Loaded typed tables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<DataTable>,
    /// Loaded table and chart views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<TableViewExport>,
    /// Parsed image metadata objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageMetadata>,
    /// Parsed annotation metadata objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<AnnotationMetadata>,
    /// Chart placements with exact source table and view metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<ChartExportItem>,
}

/// Table extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableExport {
    /// Loaded typed tables in manifest order.
    pub tables: Vec<DataTable>,
}

/// Image metadata extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageExport {
    /// Image metadata objects in manifest order.
    pub images: Vec<ImageMetadata>,
}

/// Annotation metadata extraction export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnotationExport {
    /// Annotation metadata objects in manifest order.
    pub annotations: Vec<AnnotationMetadata>,
}

/// Loaded views for one table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableViewExport {
    /// Table id that owns the views.
    pub table_id: String,
    /// Views declared on the table.
    pub views: Vec<TableView>,
}

/// Chart metadata and source-data export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartExport {
    /// Chart placements in document order.
    pub charts: Vec<ChartExportItem>,
}

/// One chart placement backed by a table view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartExportItem {
    /// Document block id for the chart placement.
    pub block_id: String,
    /// Optional placement ref from Markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_ref: Option<String>,
    /// Source table id.
    pub table_id: String,
    /// Source chart view id.
    pub view_id: String,
    /// Placement caption, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Source span of the chart placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
    /// Parsed chart view metadata.
    pub view: TableView,
    /// Exact typed source rows used by the chart.
    pub rows: Vec<TableRow>,
}

/// Schema summary export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSummaryExport {
    /// Table schema summaries in manifest order.
    pub schemas: Vec<TableSchemaSummary>,
}

/// Summary of one table schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchemaSummary {
    /// Table id.
    pub table_id: String,
    /// Schema columns.
    pub columns: Vec<TableColumnSchema>,
}

/// Agent-oriented context export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextExport {
    /// Manifest title, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Markdown entrypoint path.
    pub source_path: String,
    /// Canonical document block stream.
    pub blocks: Vec<DocumentBlock>,
    /// Table data and schemas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<DataTable>,
    /// Chart placements with source table/view references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<AgentChartContext>,
    /// Image metadata with semantic flags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<AgentImageContext>,
    /// Review annotations and proposed changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<AnnotationMetadata>,
}

/// Agent context for one chart placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChartContext {
    /// Document block id for the chart placement.
    pub block_id: String,
    /// Optional placement ref from Markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_ref: Option<String>,
    /// Source table id.
    pub table_id: String,
    /// Source chart view id.
    pub view_id: String,
    /// Chart encoding metadata.
    pub chart: serde_json::Value,
    /// Optional style metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<serde_json::Value>,
    /// Source span of the placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
}

/// Agent context for one image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentImageContext {
    /// Image id.
    pub id: String,
    /// Asset path.
    pub asset: String,
    /// Image role.
    pub role: ImageRole,
    /// Whether agents should treat the image as non-semantic.
    pub non_semantic: bool,
    /// Optional alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// Optional caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Optional meaningful visual content declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meaningful_content: Option<crate::images::MeaningfulContent>,
}

/// Build the canonical JSON export for a package.
pub fn json_export(package: &McdPackage) -> crate::Result<JsonExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    let views = load_manifest_table_views(package, &manifest)?;
    let images = crate::images::load_manifest_images(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    validate_annotation_markers(&document, &annotations)?;
    let charts = chart_export_from_parts(&document, &tables, &views)?.charts;
    Ok(JsonExport {
        manifest,
        document,
        tables: tables.into_values().collect(),
        views: views
            .into_iter()
            .map(|(table_id, table_views)| TableViewExport {
                table_id,
                views: table_views.into_values().collect(),
            })
            .collect(),
        images: images.into_values().collect(),
        annotations: annotations.into_values().collect(),
        charts,
    })
}

/// Export the original Markdown entrypoint.
pub fn original_markdown_export(package: &McdPackage) -> crate::Result<String> {
    let manifest = package.manifest()?;
    package.read_to_string(&manifest.entrypoint)
}

/// Export expanded Markdown generated from canonical blocks, tables, views, and image metadata.
pub fn expanded_markdown_export(package: &McdPackage) -> crate::Result<String> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    let views = load_manifest_table_views(package, &manifest)?;
    let images = crate::images::load_manifest_images(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    validate_annotation_markers(&document, &annotations)?;

    let mut parts = Vec::new();
    for block in &document.blocks {
        parts.push(render_expanded_block(
            block,
            &document,
            &tables,
            &views,
            &images,
            &annotations,
        )?);
    }

    Ok(parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// Build a typed table export for a package.
pub fn table_export(package: &McdPackage) -> crate::Result<TableExport> {
    let manifest = package.manifest()?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    Ok(TableExport {
        tables: tables.into_values().collect(),
    })
}

/// Build an image metadata export for a package.
pub fn image_export(package: &McdPackage) -> crate::Result<ImageExport> {
    let manifest = package.manifest()?;
    let images = crate::images::load_manifest_images(package, &manifest)?
        .into_values()
        .collect();
    Ok(ImageExport { images })
}

/// Build an annotation metadata export for a package.
pub fn annotation_export(package: &McdPackage) -> crate::Result<AnnotationExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    validate_annotation_markers(&document, &annotations)?;
    Ok(AnnotationExport {
        annotations: annotations.into_values().collect(),
    })
}

/// Build a chart metadata export for a package.
pub fn chart_export(package: &McdPackage) -> crate::Result<ChartExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    let views = load_manifest_table_views(package, &manifest)?;
    chart_export_from_parts(&document, &tables, &views)
}

/// Build a schema summary export for a package.
pub fn schema_summary_export(package: &McdPackage) -> crate::Result<SchemaSummaryExport> {
    let manifest = package.manifest()?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    Ok(SchemaSummaryExport {
        schemas: manifest
            .tables
            .iter()
            .filter_map(|entry| tables.get(&entry.id))
            .map(|table| TableSchemaSummary {
                table_id: table.id.clone(),
                columns: table.schema.columns.clone(),
            })
            .collect(),
    })
}

/// Build an agent context JSON export for a package.
pub fn agent_context_export(package: &McdPackage) -> crate::Result<AgentContextExport> {
    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = crate::tables::load_manifest_tables(package, &manifest)?;
    let views = load_manifest_table_views(package, &manifest)?;
    let images = crate::images::load_manifest_images(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    validate_annotation_markers(&document, &annotations)?;
    let chart_export = chart_export_from_parts(&document, &tables, &views)?;

    Ok(AgentContextExport {
        title: manifest.title.clone(),
        source_path: document.source_path.clone(),
        blocks: document.blocks,
        tables: tables.into_values().collect(),
        charts: chart_export
            .charts
            .into_iter()
            .map(|chart| AgentChartContext {
                block_id: chart.block_id,
                placement_ref: chart.placement_ref,
                table_id: chart.table_id,
                view_id: chart.view_id,
                chart: serde_json::to_value(chart.view.chart).unwrap_or(serde_json::Value::Null),
                style: chart.view.style,
                source: chart.source,
            })
            .collect(),
        images: images
            .into_values()
            .map(|image| AgentImageContext {
                id: image.id,
                asset: image.asset,
                role: image.role,
                non_semantic: image.role == ImageRole::Decorative,
                alt: image.alt,
                caption: image.caption,
                meaningful_content: image.meaningful_content,
            })
            .collect(),
        annotations: annotations.into_values().collect(),
    })
}

/// Load all manifest-declared table and chart views in manifest order.
pub fn load_manifest_table_views(
    package: &McdPackage,
    manifest: &Manifest,
) -> crate::Result<IndexMap<String, IndexMap<String, TableView>>> {
    let mut all_views = IndexMap::new();
    for table in &manifest.tables {
        let mut table_views = IndexMap::new();
        for (view_id, path) in &table.views {
            let view = TableView::from_package(package, path)?;
            table_views.insert(view_id.clone(), view);
        }
        all_views.insert(table.id.clone(), table_views);
    }
    Ok(all_views)
}

fn chart_export_from_parts(
    document: &McdDocument,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
) -> crate::Result<ChartExport> {
    let mut charts = Vec::new();
    for block in &document.blocks {
        let DocumentBlock::TableRef {
            id,
            placement,
            source,
        } = block
        else {
            continue;
        };
        if placement.display != TableDisplay::Chart {
            continue;
        }

        let view_id = placement.view.as_deref().ok_or_else(|| {
            export_error(
                "export.chart.view.missing",
                "Chart placement does not include a view id.",
                document,
                *source,
            )
        })?;
        let table = tables.get(&placement.table).ok_or_else(|| {
            export_error(
                "export.chart.table.missing",
                format!(
                    "Chart placement references missing table '{}'.",
                    placement.table
                ),
                document,
                *source,
            )
        })?;
        let view = views
            .get(&placement.table)
            .and_then(|table_views| table_views.get(view_id))
            .ok_or_else(|| {
                export_error(
                    "export.chart.view.missing",
                    format!(
                        "Chart placement references missing view '{}' for table '{}'.",
                        view_id, placement.table
                    ),
                    document,
                    *source,
                )
            })?;

        charts.push(ChartExportItem {
            block_id: id.clone(),
            placement_ref: placement.ref_id.clone(),
            table_id: placement.table.clone(),
            view_id: view_id.to_owned(),
            caption: placement.caption.clone(),
            source: *source,
            view: view.clone(),
            rows: table.rows.clone(),
        });
    }
    Ok(ChartExport { charts })
}

fn render_expanded_block(
    block: &DocumentBlock,
    document: &McdDocument,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
    images: &IndexMap<String, ImageMetadata>,
    annotations: &IndexMap<String, AnnotationMetadata>,
) -> crate::Result<String> {
    let markdown: crate::Result<String> = match block {
        DocumentBlock::Heading { level, text, .. } => Ok(format!(
            "{} {}",
            "#".repeat(usize::from(*level)),
            render_annotated_markdown_text(text, block.annotation_refs(), annotations)
        )),
        DocumentBlock::Paragraph { text, .. } => Ok(render_annotated_markdown_text(
            text,
            block.annotation_refs(),
            annotations,
        )),
        DocumentBlock::List { text, .. } => {
            Ok(
                render_annotated_markdown_text(text, block.annotation_refs(), annotations)
                    .lines()
                    .map(|line| format!("- {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
        DocumentBlock::CodeBlock { language, text, .. } => {
            let mut parts = vec![format!(
                "```{}\n{}\n```",
                language.as_deref().unwrap_or_default(),
                text.trim_end()
            )];
            parts.extend(block_annotation_markdown(
                block.annotation_refs(),
                annotations,
            ));
            Ok(parts.join("\n"))
        }
        DocumentBlock::Quote { text, .. } => {
            Ok(
                render_annotated_markdown_text(text, block.annotation_refs(), annotations)
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
        DocumentBlock::MathBlock { text, .. } => Ok(format!("$$\n{}\n$$", text.trim())),
        DocumentBlock::TableRef { placement, .. } => {
            let mut parts = vec![render_table_placement(placement, tables, views)?];
            parts.extend(block_annotation_markdown(
                block.annotation_refs(),
                annotations,
            ));
            Ok(parts.join("\n\n"))
        }
        DocumentBlock::ImageRef { placement, .. } => {
            let mut parts = vec![render_image_placement(placement, images)?];
            parts.extend(block_annotation_markdown(
                block.annotation_refs(),
                annotations,
            ));
            Ok(parts.join("\n\n"))
        }
    };
    let markdown = markdown?;

    Ok(append_path_annotations(
        markdown,
        block,
        document,
        annotations,
    ))
}

fn render_annotated_markdown_text(
    text: &str,
    refs: &[AnnotationRef],
    annotations: &IndexMap<String, AnnotationMetadata>,
) -> String {
    let mut inline_refs = refs
        .iter()
        .filter_map(|annotation_ref| {
            annotation_ref
                .text_offset
                .map(|offset| (offset, annotation_ref))
        })
        .collect::<Vec<_>>();
    inline_refs.sort_by_key(|(offset, _)| *offset);

    let mut markdown = String::new();
    let mut cursor = 0;
    for (offset, annotation_ref) in inline_refs {
        if offset > text.len() || offset < cursor {
            continue;
        }
        markdown.push_str(&text[cursor..offset]);
        if let Some(annotation) = annotations.get(&annotation_ref.id) {
            markdown.push_str(&annotation_markdown(annotation));
        }
        cursor = offset;
    }
    markdown.push_str(&text[cursor..]);

    let block_annotations = refs
        .iter()
        .filter(|annotation_ref| annotation_ref.text_offset.is_none())
        .filter_map(|annotation_ref| annotations.get(&annotation_ref.id))
        .map(annotation_markdown)
        .collect::<Vec<_>>();
    if !block_annotations.is_empty() {
        if !markdown.is_empty() {
            markdown.push('\n');
        }
        markdown.push_str(&block_annotations.join("\n"));
    }

    markdown
}

fn block_annotation_markdown(
    refs: &[AnnotationRef],
    annotations: &IndexMap<String, AnnotationMetadata>,
) -> Vec<String> {
    refs.iter()
        .filter_map(|annotation_ref| annotations.get(&annotation_ref.id))
        .map(annotation_markdown)
        .collect()
}

fn append_path_annotations(
    mut markdown: String,
    block: &DocumentBlock,
    document: &McdDocument,
    annotations: &IndexMap<String, AnnotationMetadata>,
) -> String {
    let path_annotations = annotations
        .values()
        .filter(|annotation| path_annotation_matches_block(annotation, block, document))
        .map(annotation_markdown)
        .collect::<Vec<_>>();
    if path_annotations.is_empty() {
        return markdown;
    }
    if !markdown.trim().is_empty() {
        markdown.push('\n');
    }
    markdown.push_str(&path_annotations.join("\n"));
    markdown
}

fn path_annotation_matches_block(
    annotation: &AnnotationMetadata,
    block: &DocumentBlock,
    document: &McdDocument,
) -> bool {
    let AnnotationTarget::Path { path, source } = &annotation.target else {
        return false;
    };
    if path != &document.source_path {
        return false;
    }
    let Some(annotation_source) = source else {
        return block_index_is_first(block);
    };
    let Some(block_source) = block_source(block) else {
        return false;
    };
    spans_overlap(*annotation_source, block_source)
}

fn block_index_is_first(block: &DocumentBlock) -> bool {
    matches!(
        block
            .id()
            .strip_prefix("block-")
            .and_then(|rest| rest.get(..4)),
        Some("0001")
    )
}

fn block_source(block: &DocumentBlock) -> Option<SourceSpan> {
    match block {
        DocumentBlock::Heading { source, .. }
        | DocumentBlock::Paragraph { source, .. }
        | DocumentBlock::List { source, .. }
        | DocumentBlock::CodeBlock { source, .. }
        | DocumentBlock::Quote { source, .. }
        | DocumentBlock::MathBlock { source, .. }
        | DocumentBlock::TableRef { source, .. }
        | DocumentBlock::ImageRef { source, .. } => *source,
    }
}

fn spans_overlap(left: SourceSpan, right: SourceSpan) -> bool {
    left.start_line <= right.end_line && right.start_line <= left.end_line
}

fn annotation_markdown(annotation: &AnnotationMetadata) -> String {
    format!(
        "(@annotation: [{}])",
        escape_annotation_text(&annotation.body)
    )
}

fn escape_annotation_text(value: &str) -> String {
    escape_markdown_text(value).replace(']', r"\]")
}

fn render_table_placement(
    placement: &TablePlacement,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
) -> crate::Result<String> {
    let table = tables.get(&placement.table).ok_or_else(|| {
        simple_export_error(
            "export.table.missing",
            format!("Table '{}' is not available for export.", placement.table),
        )
    })?;
    let view = placement
        .view
        .as_deref()
        .and_then(|view_id| views.get(&placement.table)?.get(view_id));

    let mut parts = Vec::new();
    if let Some(caption) = &placement.caption {
        parts.push(format!("**{}**", escape_markdown_text(caption)));
    }
    if placement.display == TableDisplay::Chart {
        let view_id = placement.view.as_deref().unwrap_or("default");
        let chart_type = view
            .and_then(|view| view.chart.as_ref())
            .map(|chart| format!("{:?}", chart.chart_type).to_ascii_lowercase())
            .unwrap_or_else(|| "chart".to_owned());
        parts.push(format!(
            "**Chart metadata:** table `{}`, view `{}`, type `{}`.",
            placement.table, view_id, chart_type
        ));
    }
    parts.push(markdown_table_for_placement(
        table,
        view,
        placement.display,
    )?);
    Ok(parts.join("\n\n"))
}

fn render_image_placement(
    placement: &ImagePlacement,
    images: &IndexMap<String, ImageMetadata>,
) -> crate::Result<String> {
    let image = resolve_image_placement(placement, images).ok_or_else(|| {
        simple_export_error(
            "export.image.missing",
            "Image placement does not resolve to metadata.",
        )
    })?;
    let alt = placement.alt.as_ref().or(image.alt.as_ref());
    let caption = placement.caption.as_ref().or(image.caption.as_ref());

    let mut parts = Vec::new();
    if image.role != ImageRole::Decorative {
        parts.push(format!(
            "![{}]({})",
            escape_markdown_text(alt.map(String::as_str).unwrap_or_default()),
            image.asset
        ));
    }
    if let Some(caption) = caption {
        parts.push(format!("*{}*", escape_markdown_text(caption)));
    }
    if let Some(alt) = alt {
        parts.push(format!("Alt text: {}", escape_markdown_text(alt)));
    }
    Ok(parts.join("\n\n"))
}

fn markdown_table_for_placement(
    table: &DataTable,
    view: Option<&TableView>,
    display: TableDisplay,
) -> crate::Result<String> {
    let columns = column_specs(table, view, display)?;
    let headers = columns
        .iter()
        .map(|column| escape_table_cell(&column.label))
        .collect::<Vec<_>>();
    let alignments = columns
        .iter()
        .map(|column| alignment_marker(column.column_type))
        .collect::<Vec<_>>();

    let mut lines = Vec::new();
    lines.push(format!("| {} |", headers.join(" | ")));
    lines.push(format!("| {} |", alignments.join(" | ")));
    for row in &table.rows {
        let cells = columns
            .iter()
            .map(|column| {
                row.cells
                    .get(&column.name)
                    .map(|value| escape_table_cell(&format_value(value, column)))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    Ok(lines.join("\n"))
}

fn column_specs(
    table: &DataTable,
    view: Option<&TableView>,
    display: TableDisplay,
) -> crate::Result<Vec<ColumnExportSpec>> {
    if display == TableDisplay::Chart
        && let Some(view) = view
        && let Some(chart) = &view.chart
    {
        let mut names = IndexSet::new();
        names.insert(chart.x.column.clone());
        names.insert(chart.y.column.clone());
        if let Some(series) = &chart.series {
            names.insert(series.column.clone());
        }
        if let Some(grouping) = &chart.grouping {
            names.insert(grouping.column.clone());
        }
        if let Some(mark_labels) = &chart.mark_labels
            && let Some(column) = &mark_labels.column
        {
            names.insert(column.clone());
        }
        return names
            .into_iter()
            .map(|name| {
                let encoding = encoding_for_column(view, &name);
                spec_from_schema(table, &name, encoding)
            })
            .collect();
    }

    if let Some(view) = view
        && !view.columns.is_empty()
    {
        return view
            .columns
            .iter()
            .map(|column| spec_from_view_column(table, column))
            .collect();
    }

    Ok(table
        .schema
        .columns
        .iter()
        .map(|column| spec_from_column_schema(column, None, None, None, false))
        .collect())
}

fn encoding_for_column<'a>(view: &'a TableView, name: &str) -> Option<&'a ChartEncoding> {
    let chart = view.chart.as_ref()?;
    [&chart.x, &chart.y]
        .into_iter()
        .chain(chart.series.as_ref())
        .chain(chart.grouping.as_ref())
        .find(|encoding| encoding.column == name)
}

fn spec_from_view_column(
    table: &DataTable,
    column: &ViewColumn,
) -> crate::Result<ColumnExportSpec> {
    let schema_column = table.schema.column(&column.name).ok_or_else(|| {
        simple_export_error(
            "export.view.column.missing",
            format!("View references missing column '{}'.", column.name),
        )
    })?;
    Ok(spec_from_column_schema(
        schema_column,
        column.label.as_deref(),
        column.format.as_deref(),
        column.currency.as_deref().or(column.unit.as_deref()),
        column.percent,
    ))
}

fn spec_from_schema(
    table: &DataTable,
    name: &str,
    encoding: Option<&ChartEncoding>,
) -> crate::Result<ColumnExportSpec> {
    let schema_column = table.schema.column(name).ok_or_else(|| {
        simple_export_error(
            "export.chart.column.missing",
            format!("Chart references missing column '{name}'."),
        )
    })?;
    Ok(spec_from_column_schema(
        schema_column,
        encoding.and_then(|encoding| encoding.label.as_deref()),
        encoding.and_then(|encoding| encoding.format.as_deref()),
        encoding.and_then(|encoding| encoding.currency.as_deref().or(encoding.unit.as_deref())),
        encoding.is_some_and(|encoding| encoding.percent),
    ))
}

fn spec_from_column_schema(
    column: &TableColumnSchema,
    view_label: Option<&str>,
    format: Option<&str>,
    suffix_or_currency: Option<&str>,
    percent: bool,
) -> ColumnExportSpec {
    ColumnExportSpec {
        name: column.name.clone(),
        label: view_label
            .or(column.label.as_deref())
            .unwrap_or(&column.name)
            .to_owned(),
        column_type: column.value_type,
        format: format.map(ToOwned::to_owned),
        suffix_or_currency: suffix_or_currency.map(ToOwned::to_owned),
        percent,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnExportSpec {
    name: String,
    label: String,
    column_type: ColumnType,
    format: Option<String>,
    suffix_or_currency: Option<String>,
    percent: bool,
}

fn format_value(value: &TypedValue, column: &ColumnExportSpec) -> String {
    let raw = match value {
        TypedValue::Null => return String::new(),
        TypedValue::String(value)
        | TypedValue::Decimal(value)
        | TypedValue::Date(value)
        | TypedValue::Datetime(value)
        | TypedValue::Time(value)
        | TypedValue::Enum(value) => value.clone(),
        TypedValue::Integer(value) => value.to_string(),
        TypedValue::Boolean(value) => value.to_string(),
    };

    match column.format.as_deref() {
        Some("currency") => match &column.suffix_or_currency {
            Some(currency) => format!("{currency} {raw}"),
            None => raw,
        },
        Some("percent") => format!("{raw}%"),
        Some("number" | "date" | "datetime" | "time" | "string") | None => {
            if column.percent {
                format!("{raw}%")
            } else if let Some(unit) = &column.suffix_or_currency
                && column.format.as_deref() != Some("currency")
            {
                format!("{raw} {unit}")
            } else {
                raw
            }
        }
        Some(_) => raw,
    }
}

fn alignment_marker(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::Integer | ColumnType::Decimal => "---:",
        ColumnType::Boolean => ":---:",
        ColumnType::String
        | ColumnType::Date
        | ColumnType::Datetime
        | ColumnType::Time
        | ColumnType::Enum => "---",
    }
}

fn resolve_image_placement<'a>(
    placement: &ImagePlacement,
    images: &'a IndexMap<String, ImageMetadata>,
) -> Option<&'a ImageMetadata> {
    if let Some(image_id) = &placement.image {
        return images.get(image_id);
    }
    let asset = placement.asset.as_deref()?;
    images
        .get(asset)
        .or_else(|| images.values().find(|image| image.asset == asset))
        .or_else(|| {
            images
                .values()
                .find(|image| image.asset.strip_prefix("assets/") == Some(asset))
        })
}

fn escape_table_cell(value: &str) -> String {
    escape_markdown_text(value).replace('|', r"\|")
}

fn escape_markdown_text(value: &str) -> String {
    value.replace('\n', " ")
}

fn export_error(
    code: impl Into<String>,
    message: impl Into<String>,
    document: &McdDocument,
    source: Option<SourceSpan>,
) -> McdError {
    let source = source
        .map(|span| format!("{}:{span}", document.source_path))
        .unwrap_or_else(|| document.source_path.clone());
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source))
}

fn simple_export_error(code: impl Into<String>, message: impl Into<String>) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn expanded_markdown_renders_table_and_chart_source_data() {
        let package = package_with(
            "# Revenue\n\n:::table\ntable: revenue\nview: default\ncaption: Revenue table\n:::\n\n:::table\ntable: revenue\nview: chart\ndisplay: chart\ncaption: Revenue chart\n:::\n",
        );

        let markdown = expanded_markdown_export(&package).expect("expanded markdown");

        assert!(markdown.contains("**Revenue table**"));
        assert!(markdown.contains("| Quarter | Revenue |"));
        assert!(markdown.contains("| Q1 | GBP 125000 |"));
        assert!(
            markdown.contains("**Chart metadata:** table `revenue`, view `chart`, type `bar`.")
        );
    }

    #[test]
    fn expanded_markdown_embeds_annotation_metadata() {
        let package = package_with_annotations(
            "# Revenue\n\nRevenue[[annotation:review-intro]] increased.\n\n:::table\nref: revenue-table\ntable: revenue\nannotations: review-table\n:::\n",
        );

        let markdown = expanded_markdown_export(&package).expect("expanded markdown");

        assert!(markdown.contains("Revenue(@annotation: [Review the opening copy.]) increased."));
        assert!(markdown.contains("(@annotation: [Line-level follow-up.])"));
        assert!(markdown.contains("(@annotation: [Review table totals.])"));
    }

    #[test]
    fn chart_export_contains_exact_typed_rows_and_view_refs() {
        let package = package_with(
            ":::table\nref: revenue-chart\ntable: revenue\nview: chart\ndisplay: chart\n:::\n",
        );

        let charts = chart_export(&package).expect("chart export");

        assert_eq!(charts.charts.len(), 1);
        assert_eq!(charts.charts[0].table_id, "revenue");
        assert_eq!(charts.charts[0].view_id, "chart");
        assert_eq!(charts.charts[0].rows.len(), 1);
    }

    #[test]
    fn agent_context_marks_decorative_images_non_semantic() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","images":[{"id":"logo","metadata":"images/logo.image.json"}]}"#,
            ),
            ("content/main.md", ":::image\nimage: logo\n:::\n"),
            ("assets/logo.svg", r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#),
            (
                "images/logo.image.json",
                r#"{"id":"logo","asset":"assets/logo.svg","mediaType":"image/svg+xml","role":"decorative","alt":""}"#,
            ),
        ]))
        .expect("package opens");

        let context = agent_context_export(&package).expect("agent context");

        assert_eq!(context.images.len(), 1);
        assert!(context.images[0].non_semantic);
    }

    fn package_with(markdown: &str) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            ("manifest.json", manifest()),
            ("content/main.md", markdown),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\n"),
            (
                "tables/revenue.schema.json",
                r#"{"id":"revenue","columns":[
                    {"name":"quarter","type":"string","label":"Quarter"},
                    {"name":"revenue_gbp","type":"decimal","label":"Revenue"}
                ]}"#,
            ),
            (
                "tables/revenue.view.json",
                r#"{"id":"default","table":"revenue","columns":[
                    {"name":"quarter","label":"Quarter"},
                    {"name":"revenue_gbp","label":"Revenue","format":"currency","currency":"GBP"}
                ]}"#,
            ),
            (
                "tables/revenue.chart.view.json",
                r#"{"id":"chart","table":"revenue","display":"chart","chart":{
                    "type":"bar",
                    "x":{"column":"quarter","label":"Quarter"},
                    "y":{"column":"revenue_gbp","label":"Revenue","format":"currency","currency":"GBP"}
                }}"#,
            ),
        ]))
        .expect("package opens")
    }

    fn package_with_annotations(markdown: &str) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            ("manifest.json", annotation_manifest()),
            ("content/main.md", markdown),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\n"),
            (
                "tables/revenue.schema.json",
                r#"{"id":"revenue","columns":[
                    {"name":"quarter","type":"string","label":"Quarter"},
                    {"name":"revenue_gbp","type":"decimal","label":"Revenue"}
                ]}"#,
            ),
            (
                "annotations/review-intro.annotation.json",
                r#"{"id":"review-intro","target":{"type":"document"},"kind":"comment","status":"open","body":"Review the opening copy."}"#,
            ),
            (
                "annotations/review-line.annotation.json",
                r#"{"id":"review-line","target":{"type":"path","path":"content/main.md","source":{"startLine":3,"startColumn":1,"endLine":3,"endColumn":1}},"kind":"comment","status":"open","body":"Line-level follow-up."}"#,
            ),
            (
                "annotations/review-table.annotation.json",
                r#"{"id":"review-table","target":{"type":"placement","ref":"revenue-table"},"kind":"comment","status":"open","body":"Review table totals."}"#,
            ),
        ]))
        .expect("package opens")
    }

    fn manifest() -> &'static str {
        r#"{
            "format":"MCD",
            "version":"0.1",
            "profile":"MCD-Core",
            "entrypoint":"content/main.md",
            "tables":[{
                "id":"revenue",
                "data":"tables/revenue.csv",
                "schema":"tables/revenue.schema.json",
                "views":{
                    "default":"tables/revenue.view.json",
                    "chart":"tables/revenue.chart.view.json"
                }
            }]
        }"#
    }

    fn annotation_manifest() -> &'static str {
        r#"{
            "format":"MCD",
            "version":"0.1",
            "profile":"MCD-Core",
            "entrypoint":"content/main.md",
            "tables":[{
                "id":"revenue",
                "data":"tables/revenue.csv",
                "schema":"tables/revenue.schema.json"
            }],
            "annotations":[
                {"id":"review-intro","metadata":"annotations/review-intro.annotation.json"},
                {"id":"review-line","metadata":"annotations/review-line.annotation.json"},
                {"id":"review-table","metadata":"annotations/review-table.annotation.json"}
            ]
        }"#
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
