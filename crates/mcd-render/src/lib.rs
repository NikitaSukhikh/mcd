//! HTML renderer for MCD packages.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use indexmap::{IndexMap, IndexSet};
use mcd_core::{
    Diagnostic, Manifest, McdError, McdPackage,
    annotations::{AnnotationMetadata, load_manifest_annotations},
    directives::{ImagePlacement, TableDisplay, TablePlacement},
    document::{AnnotationRef, DocumentBlock, McdDocument, SourceSpan},
    export::load_manifest_table_views,
    images::{ImageMetadata, ImageRole},
    schema::{ColumnType, TableColumnSchema},
    table_view::{ChartEncoding, ChartSpec, ChartType, TableView, ViewColumn},
    tables::{DataTable, TableRow, TypedValue, load_manifest_tables},
    validate::validate_package,
};
use serde_json::Value;

/// Render a validated MCD package to standalone semantic HTML.
pub fn render_html(package: &McdPackage) -> mcd_core::Result<String> {
    validate_package(package)?;

    let manifest = package.manifest()?;
    let document = McdDocument::from_package(package, &manifest)?;
    let tables = load_manifest_tables(package, &manifest)?;
    let views = load_manifest_table_views(package, &manifest)?;
    let images = mcd_core::images::load_manifest_images(package, &manifest)?;
    let annotations = load_manifest_annotations(package, &manifest, &document)?;
    let annotation_index = RenderAnnotationIndex::from_document(&document, &annotations);
    let styles = Styles::from_manifest(package, &manifest)?;

    let mut body = String::new();
    for block in &document.blocks {
        body.push_str(&render_block(
            package,
            block,
            &tables,
            &views,
            &images,
            &annotation_index,
            &styles,
        )?);
        body.push('\n');
    }
    body.push_str(&render_annotation_endnotes(&annotation_index));

    let title = manifest
        .title
        .clone()
        .or_else(|| document_title(&document))
        .unwrap_or_else(|| "MCD document".to_owned());

    Ok(format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<main class=\"mcd-document\" data-mcd-entrypoint=\"{}\">\n{}</main>\n</body>\n</html>\n",
        escape_html(&title),
        styles.css(),
        escape_attr(&document.source_path),
        body.trim_end()
    ))
}

fn document_title(document: &McdDocument) -> Option<String> {
    document.blocks.iter().find_map(|block| match block {
        DocumentBlock::Heading { text, .. } => Some(text.clone()),
        _ => None,
    })
}

fn render_block(
    package: &McdPackage,
    block: &DocumentBlock,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
    images: &IndexMap<String, ImageMetadata>,
    annotations: &RenderAnnotationIndex,
    styles: &Styles,
) -> mcd_core::Result<String> {
    match block {
        DocumentBlock::Heading {
            id,
            level,
            text,
            source,
            annotations: refs,
        } => {
            let level = (*level).clamp(1, 6);
            Ok(format!(
                "<h{level}{}>{}</h{level}>",
                source_attrs(id, *source),
                render_annotated_text(text, refs, annotations)
            ))
        }
        DocumentBlock::Paragraph {
            id,
            text,
            source,
            annotations: refs,
        } => Ok(format!(
            "<p{}>{}</p>",
            source_attrs(id, *source),
            render_annotated_text(text, refs, annotations)
        )),
        DocumentBlock::List {
            id,
            text,
            source,
            annotations: refs,
        } => {
            let items = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| format!("<li>{}</li>", escape_html(line.trim())))
                .collect::<String>()
                + &render_block_annotation_markers(refs, annotations);
            Ok(format!("<ul{}>{items}</ul>", source_attrs(id, *source)))
        }
        DocumentBlock::CodeBlock {
            id,
            language,
            text,
            source,
            ..
        } => {
            let class = language
                .as_deref()
                .map(|language| format!(" class=\"language-{}\"", escape_attr(language)))
                .unwrap_or_default();
            Ok(format!(
                "<pre{}><code{}>{}</code></pre>",
                source_attrs(id, *source),
                class,
                escape_html(text)
            ))
        }
        DocumentBlock::Quote {
            id,
            text,
            source,
            annotations: refs,
        } => Ok(format!(
            "<blockquote{}>{}</blockquote>",
            source_attrs(id, *source),
            text.lines()
                .map(|line| format!("<p>{}</p>", escape_html(line.trim())))
                .collect::<String>()
                + &render_block_annotation_markers(refs, annotations)
        )),
        DocumentBlock::MathBlock { id, text, source } => Ok(format!(
            "<pre{} class=\"mcd-math\"><code>{}</code></pre>",
            source_attrs(id, *source),
            escape_html(text.trim())
        )),
        DocumentBlock::TableRef {
            id,
            placement,
            source,
        } => render_table_or_chart(id, *source, placement, tables, views, annotations, styles),
        DocumentBlock::ImageRef {
            id,
            placement,
            source,
        } => render_image(package, id, *source, placement, images, annotations),
    }
}

fn render_table_or_chart(
    block_id: &str,
    source: Option<SourceSpan>,
    placement: &TablePlacement,
    tables: &IndexMap<String, DataTable>,
    views: &IndexMap<String, IndexMap<String, TableView>>,
    annotations: &RenderAnnotationIndex,
    styles: &Styles,
) -> mcd_core::Result<String> {
    let table = tables.get(&placement.table).ok_or_else(|| {
        render_error(
            "render.table.missing",
            format!(
                "Table '{}' is not available for rendering.",
                placement.table
            ),
        )
    })?;
    let view = placement
        .view
        .as_deref()
        .and_then(|view_id| views.get(&placement.table)?.get(view_id));

    match placement.display {
        TableDisplay::Table => render_table(block_id, source, placement, table, view, annotations),
        TableDisplay::Chart => render_chart(
            block_id,
            source,
            placement,
            table,
            view,
            annotations,
            styles,
        ),
    }
}

fn render_table(
    block_id: &str,
    source: Option<SourceSpan>,
    placement: &TablePlacement,
    table: &DataTable,
    view: Option<&TableView>,
    annotations: &RenderAnnotationIndex,
) -> mcd_core::Result<String> {
    let columns = table_columns(table, view)?;
    let mut html = format!(
        "<figure{} class=\"mcd-table-figure\" data-mcd-table-id=\"{}\"{}>",
        source_attrs(block_id, source),
        escape_attr(&table.id),
        placement_ref_attr(placement.ref_id.as_deref())
    );
    html.push_str(&render_block_annotation_markers(
        &placement.annotations,
        annotations,
    ));
    if let Some(caption) = &placement.caption {
        html.push_str(&format!(
            "<figcaption>{}</figcaption>",
            escape_html(caption)
        ));
    }
    html.push_str("<table><thead><tr>");
    for column in &columns {
        html.push_str(&format!(
            "<th scope=\"col\" data-mcd-column=\"{}\"{}>{}</th>",
            escape_attr(&column.name),
            align_attr(column.column_type),
            escape_html(&column.label)
        ));
    }
    html.push_str("</tr></thead><tbody>");
    for row in &table.rows {
        html.push_str("<tr>");
        for column in &columns {
            let cell = row
                .cells
                .get(&column.name)
                .map(|value| format_value(value, column))
                .unwrap_or_default();
            html.push_str(&format!(
                "<td{}>{}</td>",
                align_attr(column.column_type),
                escape_html(&cell)
            ));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></figure>");
    Ok(html)
}

fn render_image(
    package: &McdPackage,
    block_id: &str,
    source: Option<SourceSpan>,
    placement: &ImagePlacement,
    images: &IndexMap<String, ImageMetadata>,
    annotations: &RenderAnnotationIndex,
) -> mcd_core::Result<String> {
    let image = resolve_image_placement(placement, images).ok_or_else(|| {
        render_error(
            "render.image.missing",
            "Image placement does not resolve to metadata.",
        )
    })?;
    let bytes = package.read(&image.asset)?;
    let src = format!(
        "data:{};base64,{}",
        image.media_type,
        STANDARD.encode(bytes)
    );
    let alt = placement
        .alt
        .as_deref()
        .or(image.alt.as_deref())
        .unwrap_or_default();
    let caption = placement.caption.as_ref().or(image.caption.as_ref());
    let size_attrs = image
        .intrinsic_size
        .as_ref()
        .map(|size| format!(" width=\"{}\" height=\"{}\"", size.width, size.height))
        .unwrap_or_default();
    let decorative_attrs = if image.role == ImageRole::Decorative {
        " aria-hidden=\"true\""
    } else {
        ""
    };

    let mut html = format!(
        "<figure{} class=\"mcd-image-figure\" data-mcd-image-id=\"{}\" data-mcd-asset=\"{}\"{}>",
        source_attrs(block_id, source),
        escape_attr(&image.id),
        escape_attr(&image.asset),
        placement_ref_attr(placement.ref_id.as_deref())
    );
    html.push_str(&render_block_annotation_markers(
        &placement.annotations,
        annotations,
    ));
    html.push_str(&format!(
        "<img src=\"{}\" alt=\"{}\"{}{}>",
        escape_attr(&src),
        escape_attr(alt),
        size_attrs,
        decorative_attrs
    ));
    if let Some(caption) = caption {
        html.push_str(&format!(
            "<figcaption>{}</figcaption>",
            escape_html(caption)
        ));
    }
    html.push_str("</figure>");
    Ok(html)
}

fn render_chart(
    block_id: &str,
    source: Option<SourceSpan>,
    placement: &TablePlacement,
    table: &DataTable,
    view: Option<&TableView>,
    annotations: &RenderAnnotationIndex,
    styles: &Styles,
) -> mcd_core::Result<String> {
    let view = view.ok_or_else(|| {
        render_error(
            "render.chart.view.missing",
            "Chart placement does not resolve to a chart view.",
        )
    })?;
    let chart = view.chart.as_ref().ok_or_else(|| {
        render_error(
            "render.chart.spec.missing",
            format!("View '{}' does not include chart metadata.", view.id),
        )
    })?;
    let svg = chart_svg(table, view, chart, styles)?;
    let mut html = format!(
        "<figure{} class=\"mcd-chart-figure\" data-mcd-table-id=\"{}\" data-mcd-view-id=\"{}\"{}>",
        source_attrs(block_id, source),
        escape_attr(&table.id),
        escape_attr(&view.id),
        placement_ref_attr(placement.ref_id.as_deref())
    );
    html.push_str(&render_block_annotation_markers(
        &placement.annotations,
        annotations,
    ));
    if let Some(caption) = &placement.caption {
        html.push_str(&format!(
            "<figcaption>{}</figcaption>",
            escape_html(caption)
        ));
    }
    html.push_str(&svg);
    html.push_str("</figure>");
    Ok(html)
}

fn chart_svg(
    table: &DataTable,
    view: &TableView,
    chart: &ChartSpec,
    styles: &Styles,
) -> mcd_core::Result<String> {
    let width = style_number(view.style.as_ref(), "width")
        .or_else(|| styles.chart_width())
        .unwrap_or(720.0);
    let height = style_number(view.style.as_ref(), "height")
        .or_else(|| styles.chart_height())
        .unwrap_or(360.0);
    let palette = chart_palette(view.style.as_ref(), styles);
    let color = palette
        .first()
        .cloned()
        .unwrap_or_else(|| "#2563eb".to_owned());
    let stroke = style_string(view.style.as_ref(), "stroke")
        .or_else(|| style_string(view.style.as_ref(), "color"))
        .or_else(|| styles.chart_color())
        .unwrap_or_else(|| color.clone());

    let margin_left = 64.0;
    let margin_right = 28.0;
    let margin_top = 24.0;
    let margin_bottom = 56.0;
    let plot_width = (width - margin_left - margin_right).max(1.0);
    let plot_height = (height - margin_top - margin_bottom).max(1.0);
    let baseline = margin_top + plot_height;

    let y_values = table
        .rows
        .iter()
        .map(|row| numeric_cell(row, &chart.y.column))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            render_error(
                "render.chart.value.invalid",
                "Chart y values must be numeric.",
            )
        })?;
    let max_y = y_values.iter().copied().fold(0.0_f64, f64::max).max(1.0);

    let points = table
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let x = match chart.chart_type {
                ChartType::Scatter => numeric_cell(row, &chart.x.column)
                    .map(|value| {
                        margin_left
                            + value_position(value, x_min_max(table, &chart.x.column), plot_width)
                    })
                    .unwrap_or_else(|| {
                        categorical_x(index, table.rows.len(), margin_left, plot_width)
                    }),
                ChartType::Bar | ChartType::Line | ChartType::Area => {
                    categorical_x(index, table.rows.len(), margin_left, plot_width)
                }
            };
            let y = baseline - (y_values[index] / max_y * plot_height);
            (x, y)
        })
        .collect::<Vec<_>>();

    let mut svg = format!(
        "<svg class=\"mcd-chart\" role=\"img\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">",
        format_number(width),
        format_number(height),
        format_number(width),
        format_number(height)
    );
    svg.push_str(&format!(
        "<title>{}</title><desc>Chart generated from table '{}' and view '{}'.</desc>",
        escape_html(&chart_title(chart)),
        escape_html(&table.id),
        escape_html(&view.id)
    ));
    svg.push_str(&format!(
        "<line class=\"mcd-chart-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/><line class=\"mcd-chart-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        format_number(margin_left),
        format_number(margin_top),
        format_number(margin_left),
        format_number(baseline),
        format_number(margin_left),
        format_number(baseline),
        format_number(width - margin_right),
        format_number(baseline)
    ));
    svg.push_str(&format!(
        "<text class=\"mcd-chart-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
        format_number(margin_left + plot_width / 2.0),
        format_number(height - 12.0),
        escape_html(encoding_label(&chart.x))
    ));
    svg.push_str(&format!(
        "<text class=\"mcd-chart-label\" transform=\"translate(16 {}) rotate(-90)\" text-anchor=\"middle\">{}</text>",
        format_number(margin_top + plot_height / 2.0),
        escape_html(encoding_label(&chart.y))
    ));

    match chart.chart_type {
        ChartType::Bar => {
            let step = plot_width / table.rows.len().max(1) as f64;
            let bar_width = (step * 0.64).max(1.0);
            for (index, (x, y)) in points.iter().enumerate() {
                let value = y_values[index];
                svg.push_str(&format!(
                    "<rect class=\"mcd-chart-mark\" data-mcd-row=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"><title>{}: {}</title></rect>",
                    index,
                    format_number(x - bar_width / 2.0),
                    format_number(*y),
                    format_number(bar_width),
                    format_number(baseline - *y),
                    escape_attr(&color),
                    escape_html(x_label(&table.rows[index], &chart.x.column)),
                    escape_html(&format_encoded_value(value, &chart.y))
                ));
            }
        }
        ChartType::Line | ChartType::Area => {
            let point_data = points
                .iter()
                .map(|(x, y)| format!("{},{}", format_number(*x), format_number(*y)))
                .collect::<Vec<_>>()
                .join(" ");
            if chart.chart_type == ChartType::Area {
                let area = format!(
                    "{},{} {} {},{}",
                    format_number(points.first().map_or(margin_left, |point| point.0)),
                    format_number(baseline),
                    point_data,
                    format_number(points.last().map_or(margin_left, |point| point.0)),
                    format_number(baseline)
                );
                svg.push_str(&format!(
                    "<polygon class=\"mcd-chart-area\" points=\"{}\" fill=\"{}\" opacity=\"0.22\"/>",
                    escape_attr(&area),
                    escape_attr(&color)
                ));
            }
            svg.push_str(&format!(
                "<polyline class=\"mcd-chart-line\" points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
                escape_attr(&point_data),
                escape_attr(&stroke)
            ));
            for (index, (x, y)) in points.iter().enumerate() {
                svg.push_str(&format!(
                    "<circle class=\"mcd-chart-point\" data-mcd-row=\"{}\" cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\"><title>{}: {}</title></circle>",
                    index,
                    format_number(*x),
                    format_number(*y),
                    escape_attr(&color),
                    escape_html(x_label(&table.rows[index], &chart.x.column)),
                    escape_html(&format_encoded_value(y_values[index], &chart.y))
                ));
            }
        }
        ChartType::Scatter => {
            for (index, (x, y)) in points.iter().enumerate() {
                svg.push_str(&format!(
                    "<circle class=\"mcd-chart-point\" data-mcd-row=\"{}\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{}\"><title>{}: {}</title></circle>",
                    index,
                    format_number(*x),
                    format_number(*y),
                    escape_attr(&color),
                    escape_html(x_label(&table.rows[index], &chart.x.column)),
                    escape_html(&format_encoded_value(y_values[index], &chart.y))
                ));
            }
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

fn x_min_max(table: &DataTable, column: &str) -> (f64, f64) {
    let values = table
        .rows
        .iter()
        .filter_map(|row| numeric_cell(row, column))
        .collect::<Vec<_>>();
    let min = values.iter().copied().fold(0.0_f64, f64::min);
    let max = values
        .iter()
        .copied()
        .fold(1.0_f64, f64::max)
        .max(min + 1.0);
    (min, max)
}

fn value_position(value: f64, (min, max): (f64, f64), width: f64) -> f64 {
    ((value - min) / (max - min) * width).clamp(0.0, width)
}

fn categorical_x(index: usize, count: usize, left: f64, width: f64) -> f64 {
    if count <= 1 {
        return left + width / 2.0;
    }
    left + (index as f64 + 0.5) * (width / count as f64)
}

fn numeric_cell(row: &TableRow, column: &str) -> Option<f64> {
    match row.cells.get(column)? {
        TypedValue::Integer(value) => Some(*value as f64),
        TypedValue::Decimal(value) => value.parse::<f64>().ok(),
        _ => None,
    }
}

fn x_label<'a>(row: &'a TableRow, column: &str) -> &'a str {
    match row.cells.get(column) {
        Some(TypedValue::String(value))
        | Some(TypedValue::Decimal(value))
        | Some(TypedValue::Date(value))
        | Some(TypedValue::Datetime(value))
        | Some(TypedValue::Time(value))
        | Some(TypedValue::Enum(value)) => value,
        Some(TypedValue::Integer(_))
        | Some(TypedValue::Boolean(_))
        | Some(TypedValue::Null)
        | None => "",
    }
}

fn chart_title(chart: &ChartSpec) -> String {
    format!(
        "{} by {}",
        encoding_label(&chart.y),
        encoding_label(&chart.x)
    )
}

fn encoding_label(encoding: &ChartEncoding) -> &str {
    encoding.label.as_deref().unwrap_or(&encoding.column)
}

fn format_encoded_value(value: f64, encoding: &ChartEncoding) -> String {
    let raw = format_number(value);
    match encoding.format.as_deref() {
        Some("currency") => encoding
            .currency
            .as_ref()
            .map(|currency| format!("{currency} {raw}"))
            .unwrap_or(raw),
        Some("percent") => format!("{raw}%"),
        _ if encoding.percent => format!("{raw}%"),
        _ => encoding
            .unit
            .as_ref()
            .map(|unit| format!("{raw} {unit}"))
            .unwrap_or(raw),
    }
}

fn table_columns(table: &DataTable, view: Option<&TableView>) -> mcd_core::Result<Vec<ColumnSpec>> {
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
        .map(|column| spec_from_schema_column(column, None, None, None, false))
        .collect())
}

fn spec_from_view_column(table: &DataTable, column: &ViewColumn) -> mcd_core::Result<ColumnSpec> {
    let schema_column = table.schema.column(&column.name).ok_or_else(|| {
        render_error(
            "render.view.column.missing",
            format!("View references missing column '{}'.", column.name),
        )
    })?;
    Ok(spec_from_schema_column(
        schema_column,
        column.label.as_deref(),
        column.format.as_deref(),
        column.currency.as_deref().or(column.unit.as_deref()),
        column.percent,
    ))
}

fn spec_from_schema_column(
    column: &TableColumnSchema,
    label: Option<&str>,
    format: Option<&str>,
    suffix_or_currency: Option<&str>,
    percent: bool,
) -> ColumnSpec {
    ColumnSpec {
        name: column.name.clone(),
        label: label
            .or(column.label.as_deref())
            .unwrap_or(&column.name)
            .to_owned(),
        column_type: column.value_type,
        format: format.map(ToOwned::to_owned),
        suffix_or_currency: suffix_or_currency.map(ToOwned::to_owned),
        percent,
    }
}

#[derive(Debug, Clone)]
struct ColumnSpec {
    name: String,
    label: String,
    column_type: ColumnType,
    format: Option<String>,
    suffix_or_currency: Option<String>,
    percent: bool,
}

fn format_value(value: &TypedValue, column: &ColumnSpec) -> String {
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
        Some("currency") => column
            .suffix_or_currency
            .as_ref()
            .map(|currency| format!("{currency} {raw}"))
            .unwrap_or(raw),
        Some("percent") => format!("{raw}%"),
        _ if column.percent => format!("{raw}%"),
        _ => column
            .suffix_or_currency
            .as_ref()
            .filter(|_| column.format.as_deref() != Some("currency"))
            .map(|unit| format!("{raw} {unit}"))
            .unwrap_or(raw),
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

#[derive(Debug, Clone)]
struct RenderAnnotation {
    number: usize,
    metadata: AnnotationMetadata,
}

#[derive(Debug, Clone, Default)]
struct RenderAnnotationIndex {
    by_id: IndexMap<String, RenderAnnotation>,
    ordered_ids: Vec<String>,
}

impl RenderAnnotationIndex {
    fn from_document(
        document: &McdDocument,
        annotations: &IndexMap<String, AnnotationMetadata>,
    ) -> Self {
        let mut index = Self::default();
        for block in &document.blocks {
            for annotation_ref in block.annotation_refs() {
                if let Some(metadata) = annotations.get(&annotation_ref.id) {
                    index.insert(metadata.clone());
                }
            }
        }
        index
    }

    fn insert(&mut self, metadata: AnnotationMetadata) {
        if self.by_id.contains_key(&metadata.id) {
            return;
        }
        let number = self.ordered_ids.len() + 1;
        self.ordered_ids.push(metadata.id.clone());
        self.by_id
            .insert(metadata.id.clone(), RenderAnnotation { number, metadata });
    }

    fn get(&self, id: &str) -> Option<&RenderAnnotation> {
        self.by_id.get(id)
    }
}

fn render_annotated_text(
    text: &str,
    refs: &[AnnotationRef],
    annotations: &RenderAnnotationIndex,
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

    let mut html = String::new();
    let mut cursor = 0;
    for (offset, annotation_ref) in inline_refs {
        if offset > text.len() || offset < cursor {
            continue;
        }
        html.push_str(&escape_html(&text[cursor..offset]));
        html.push_str(&render_annotation_marker(&annotation_ref.id, annotations));
        cursor = offset;
    }
    html.push_str(&escape_html(&text[cursor..]));

    let block_refs = refs
        .iter()
        .filter(|annotation_ref| annotation_ref.text_offset.is_none())
        .cloned()
        .collect::<Vec<_>>();
    html.push_str(&render_block_annotation_markers(&block_refs, annotations));
    html
}

fn render_block_annotation_markers(
    refs: &[AnnotationRef],
    annotations: &RenderAnnotationIndex,
) -> String {
    refs.iter()
        .map(|annotation_ref| render_annotation_marker(&annotation_ref.id, annotations))
        .collect::<String>()
}

fn render_annotation_marker(id: &str, annotations: &RenderAnnotationIndex) -> String {
    let Some(annotation) = annotations.get(id) else {
        return String::new();
    };
    format!(
        "<sup class=\"mcd-annotation-marker\"><a href=\"#mcd-annotation-{}\" aria-label=\"Annotation {}\">{}</a></sup>",
        escape_attr(id),
        annotation.number,
        annotation.number
    )
}

fn render_annotation_endnotes(annotations: &RenderAnnotationIndex) -> String {
    if annotations.ordered_ids.is_empty() {
        return String::new();
    }

    let mut html =
        "\n<section class=\"mcd-annotations\" aria-label=\"Annotations\">\n<h2>Annotations</h2>\n<ol>\n"
            .to_owned();
    for id in &annotations.ordered_ids {
        let Some(annotation) = annotations.get(id) else {
            continue;
        };
        html.push_str(&format!(
            "<li id=\"mcd-annotation-{}\"><span class=\"mcd-annotation-kind\">{}</span>: {}</li>\n",
            escape_attr(id),
            escape_html(&format!("{:?}", annotation.metadata.kind).to_ascii_lowercase()),
            escape_html(&annotation.metadata.body)
        ));
    }
    html.push_str("</ol>\n</section>\n");
    html
}

#[derive(Debug, Clone, Default)]
struct Styles {
    value: Option<Value>,
}

impl Styles {
    fn from_manifest(package: &McdPackage, manifest: &Manifest) -> mcd_core::Result<Self> {
        let Some(styles_path) = manifest
            .layout
            .as_ref()
            .and_then(|layout| layout.styles.as_ref())
        else {
            return Ok(Self::default());
        };
        let value = serde_json::from_slice::<Value>(package.read(styles_path)?)?;
        Ok(Self { value: Some(value) })
    }

    fn css(&self) -> String {
        let mut css = default_css();
        if let Some(value) = &self.value {
            css.push_str(&self.css_variables(value));
            css.push_str(&self.style_rule(value, "body", "body"));
            css.push_str(&self.style_rule(value, "tables", ".mcd-table-figure table"));
            css.push_str(&self.style_rule(value, "charts", ".mcd-chart-figure"));
            css.push_str(&self.style_rule(value, "images", ".mcd-image-figure"));
            css.push_str(&self.style_rule(value, "page", ".mcd-document"));
        }
        css
    }

    fn css_variables(&self, value: &Value) -> String {
        let mut vars = String::new();
        if let Some(colors) = value.get("colors").and_then(Value::as_object) {
            for (name, color) in colors {
                if let Some(color) = color.as_str() {
                    vars.push_str(&format!(
                        "--mcd-color-{}: {};\n",
                        css_ident(name),
                        sanitize_css_value(color)
                    ));
                }
            }
        }
        if let Some(fonts) = value.get("fonts").and_then(Value::as_object) {
            for (name, font) in fonts {
                if let Some(font) = font.as_str() {
                    vars.push_str(&format!(
                        "--mcd-font-{}: {};\n",
                        css_ident(name),
                        sanitize_css_value(font)
                    ));
                }
            }
        }
        if vars.is_empty() {
            String::new()
        } else {
            format!(":root {{\n{vars}}}\n")
        }
    }

    fn style_rule(&self, value: &Value, key: &str, selector: &str) -> String {
        let Some(object) = value.get(key).and_then(Value::as_object) else {
            return String::new();
        };
        let declarations = object
            .iter()
            .filter_map(|(name, value)| style_declaration(name, value))
            .collect::<Vec<_>>();
        if declarations.is_empty() {
            String::new()
        } else {
            format!("{selector} {{\n{}\n}}\n", declarations.join("\n"))
        }
    }

    fn chart_width(&self) -> Option<f64> {
        self.value
            .as_ref()
            .and_then(|value| style_number(value.get("charts"), "width"))
    }

    fn chart_height(&self) -> Option<f64> {
        self.value
            .as_ref()
            .and_then(|value| style_number(value.get("charts"), "height"))
    }

    fn chart_color(&self) -> Option<String> {
        self.value.as_ref().and_then(|value| {
            value
                .get("charts")
                .and_then(|charts| style_string(Some(charts), "color"))
                .or_else(|| {
                    value
                        .pointer("/colors/accent")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
        })
    }
}

fn default_css() -> String {
    r#":root {
--mcd-color-text: #111827;
--mcd-color-background: #ffffff;
--mcd-color-border: #d1d5db;
--mcd-color-muted: #6b7280;
--mcd-color-accent: #2563eb;
}
body {
margin: 0;
font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
line-height: 1.5;
color: var(--mcd-color-text);
background: var(--mcd-color-background);
}
.mcd-document {
max-width: 880px;
margin: 0 auto;
padding: 32px 24px;
}
[data-mcd-source-id] {
scroll-margin-top: 24px;
}
figure {
margin: 24px 0;
}
figcaption {
font-weight: 600;
margin-bottom: 8px;
}
.mcd-table-figure table {
width: 100%;
border-collapse: collapse;
font-variant-numeric: tabular-nums;
}
.mcd-table-figure th,
.mcd-table-figure td {
border: 1px solid var(--mcd-color-border);
padding: 8px 10px;
vertical-align: top;
}
.mcd-table-figure th {
background: #f9fafb;
text-align: left;
}
.mcd-image-figure img {
max-width: 100%;
height: auto;
display: block;
}
.mcd-chart {
max-width: 100%;
height: auto;
display: block;
}
.mcd-chart-axis {
stroke: var(--mcd-color-border);
stroke-width: 1;
}
.mcd-chart-label {
fill: var(--mcd-color-muted);
font-size: 13px;
}
.mcd-annotation-marker {
opacity: 0.5;
font-size: 0.72em;
line-height: 0;
margin-left: 0.12em;
vertical-align: super;
}
.mcd-annotation-marker a {
color: inherit;
text-decoration: none;
}
.mcd-annotations {
break-before: page;
page-break-before: always;
margin-top: 48px;
opacity: 0.5;
font-size: 0.9em;
}
.mcd-annotations h2 {
font-size: 1.1em;
margin-bottom: 12px;
}
.mcd-annotations li {
margin: 6px 0;
}
.mcd-annotation-kind {
font-weight: 600;
}
"#
    .to_owned()
}

fn style_declaration(name: &str, value: &Value) -> Option<String> {
    let property = match name {
        "fontFamily" => "font-family",
        "fontSize" => "font-size",
        "color" => "color",
        "background" => "background",
        "backgroundColor" => "background-color",
        "maxWidth" => "max-width",
        "margin" => "margin",
        "padding" => "padding",
        "borderColor" => "border-color",
        "width" => "width",
        "height" => "height",
        "textAlign" => "text-align",
        _ => return None,
    };
    let value = match value {
        Value::String(value) => sanitize_css_value(value),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null | Value::Array(_) | Value::Object(_) => return None,
    };
    Some(format!("{property}: {value};"))
}

fn chart_palette(view_style: Option<&Value>, styles: &Styles) -> Vec<String> {
    let mut colors = IndexSet::new();
    if let Some(style) = view_style {
        if let Some(color) = style_string(Some(style), "color") {
            colors.insert(color);
        }
        if let Some(color) = style_string(Some(style), "fill") {
            colors.insert(color);
        }
        if let Some(values) = style.get("colors").and_then(Value::as_array) {
            for value in values {
                if let Some(color) = value.as_str() {
                    colors.insert(color.to_owned());
                }
            }
        }
    }
    if let Some(color) = styles.chart_color() {
        colors.insert(color);
    }
    colors.into_iter().collect()
}

fn style_string(style: Option<&Value>, key: &str) -> Option<String> {
    style?
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn style_number(style: Option<&Value>, key: &str) -> Option<f64> {
    let value = style?.get(key)?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(parse_css_number))
}

fn parse_css_number(value: &str) -> Option<f64> {
    let numeric = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    numeric.parse::<f64>().ok()
}

fn sanitize_css_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '<' | '>' | '"' | '\'' | ';' | '{' | '}'))
        .collect()
}

fn css_ident(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn source_attrs(id: &str, source: Option<SourceSpan>) -> String {
    let mut attrs = format!(
        " id=\"{}\" data-mcd-source-id=\"{}\"",
        escape_attr(id),
        escape_attr(id)
    );
    if let Some(source) = source {
        attrs.push_str(&format!(
            " data-mcd-source=\"{}\"",
            escape_attr(&source.to_string())
        ));
    }
    attrs
}

fn placement_ref_attr(ref_id: Option<&str>) -> String {
    ref_id
        .map(|ref_id| format!(" data-mcd-ref=\"{}\"", escape_attr(ref_id)))
        .unwrap_or_default()
}

fn align_attr(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::Integer | ColumnType::Decimal => " data-align=\"right\"",
        ColumnType::Boolean => " data-align=\"center\"",
        ColumnType::String
        | ColumnType::Date
        | ColumnType::Datetime
        | ColumnType::Time
        | ColumnType::Enum => "",
    }
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value).replace('"', "&quot;")
}

fn render_error(code: impl Into<String>, message: impl Into<String>) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn renders_table_image_and_chart_html() {
        let package = package_with(
            "# Revenue\n\n:::table\nref: revenue-table\ntable: revenue\nview: default\ncaption: Revenue table\n:::\n\n:::table\nref: revenue-chart\ntable: revenue\nview: chart\ndisplay: chart\ncaption: Revenue chart\n:::\n\n:::image\nref: logo-placement\nimage: logo\n:::\n",
        );

        let html = render_html(&package).expect("html renders");

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("data-mcd-ref=\"revenue-table\""));
        assert!(html.contains(
            "<th scope=\"col\" data-mcd-column=\"revenue_gbp\" data-align=\"right\">Revenue</th>"
        ));
        assert!(html.contains("<td data-align=\"right\">GBP 125000</td>"));
        assert!(html.contains("data-mcd-ref=\"revenue-chart\""));
        assert!(html.contains("<svg class=\"mcd-chart\" role=\"img\""));
        assert!(html.contains("<rect class=\"mcd-chart-mark\""));
        assert!(html.contains("src=\"data:image/svg+xml;base64,"));
        assert!(html.contains("alt=\"Logo\""));
    }

    #[test]
    fn renders_stable_minimal_snapshot() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", mcd_core::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","title":"Snapshot"}"#,
            ),
            ("content/main.md", "# Snapshot\n\nPlain <text>.\n"),
        ]))
        .expect("package opens");

        let html = render_html(&package).expect("html renders");
        let stable_lines = html
            .lines()
            .filter(|line| {
                line.starts_with("<title>")
                    || line.starts_with("<main")
                    || line.starts_with("<h1")
                    || line.starts_with("<p")
            })
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(stable_lines, @r###"
        <title>Snapshot</title>
        <main class="mcd-document" data-mcd-entrypoint="content/main.md">
        <h1 id="block-0001-heading" data-mcd-source-id="block-0001-heading" data-mcd-source="1:1-1:10">Snapshot</h1>
        <p id="block-0002-paragraph" data-mcd-source-id="block-0002-paragraph" data-mcd-source="3:1-3:13">Plain &lt;text&gt;.</p></main>
        "###);
    }

    #[test]
    fn renders_annotation_markers_and_endnotes() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", mcd_core::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{
                    "format":"MCD",
                    "version":"0.1",
                    "profile":"MCD-Core",
                    "entrypoint":"content/main.md",
                    "annotations":[
                        {"id":"review-intro","metadata":"annotations/review-intro.annotation.json"},
                        {"id":"review-table","metadata":"annotations/review-table.annotation.json"}
                    ],
                    "tables":[{"id":"revenue","data":"tables/revenue.csv","schema":"tables/revenue.schema.json"}]
                }"#,
            ),
            (
                "content/main.md",
                "Revenue[[annotation:review-intro]] increased.\n\n:::table\nref: revenue-table\ntable: revenue\nannotations: review-table\n:::\n",
            ),
            (
                "annotations/review-intro.annotation.json",
                r#"{"id":"review-intro","target":{"type":"document"},"kind":"comment","status":"open","body":"Confirm this wording."}"#,
            ),
            (
                "annotations/review-table.annotation.json",
                r#"{"id":"review-table","target":{"type":"placement","ref":"revenue-table"},"kind":"flag","status":"open","body":"Check table source."}"#,
            ),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\n"),
            (
                "tables/revenue.schema.json",
                r#"{"id":"revenue","columns":[{"name":"quarter","type":"string"},{"name":"revenue_gbp","type":"decimal"}]}"#,
            ),
        ]))
        .expect("package opens");

        let html = render_html(&package).expect("html renders");

        assert!(html.contains("Revenue<sup class=\"mcd-annotation-marker\""));
        assert!(html.contains("href=\"#mcd-annotation-review-intro\""));
        assert!(html.contains("<section class=\"mcd-annotations\""));
        assert!(html.contains("Confirm this wording."));
        assert!(html.contains("Check table source."));
    }

    fn package_with(markdown: &str) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", mcd_core::package::MCD_MIMETYPE),
            ("manifest.json", manifest()),
            ("content/main.md", markdown),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\nQ2,150000.00\n"),
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
                r##"{"id":"chart","table":"revenue","display":"chart","chart":{
                    "type":"bar",
                    "x":{"column":"quarter","label":"Quarter"},
                    "y":{"column":"revenue_gbp","label":"Revenue","format":"currency","currency":"GBP"}
                },"style":{"color":"#0f766e","width":640,"height":320}}"##,
            ),
            (
                "images/logo.image.json",
                r#"{"id":"logo","asset":"assets/logo.svg","mediaType":"image/svg+xml","role":"logo","alt":"Logo"}"#,
            ),
            ("assets/logo.svg", r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#),
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
            }],
            "images":[{"id":"logo","metadata":"images/logo.image.json"}],
            "assets":[]
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
