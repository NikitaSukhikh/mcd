use std::path::Path;

use anyhow::{Result, bail};
use mcd_core::{
    McdPackage,
    annotations::{AnnotationMetadata, AnnotationTarget},
    export::{
        AnnotationExport, annotation_export, chart_export, expanded_markdown_export, image_export,
        json_export, original_markdown_export, table_export,
    },
};
use serde_json::json;

pub enum ExportMode {
    Annotations,
}

pub struct ExtractOptions<'a> {
    pub(crate) export: Option<ExportMode>,
    pub(crate) json: bool,
    pub(crate) markdown: bool,
    pub(crate) expand_tables: bool,
    pub(crate) tables: bool,
    pub(crate) images: bool,
    pub(crate) annotations: bool,
    pub(crate) page: Option<&'a str>,
    pub(crate) line: Option<usize>,
    pub(crate) charts: bool,
}

pub fn run(file: &Path, options: ExtractOptions<'_>) -> Result<()> {
    let ExtractOptions {
        export,
        json,
        markdown,
        expand_tables,
        tables,
        images,
        annotations,
        page,
        line,
        charts,
    } = options;

    let export_annotations = matches!(export, Some(ExportMode::Annotations));
    let annotations = annotations || export_annotations;

    let modes = [json, markdown, tables, images, annotations, charts]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if modes != 1 {
        bail!(
            "choose exactly one extraction mode: --json, --markdown, --tables, --images, --annotations, or --charts"
        );
    }
    if expand_tables && !markdown {
        bail!("--expand-tables can only be used with --markdown");
    }
    if (page.is_some() || line.is_some()) && !annotations {
        bail!("--page and --line can only be used with annotation export");
    }
    if line == Some(0) {
        bail!("annotation line filter must be 1 or greater");
    }

    let package = McdPackage::open_path(file)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&json_export(&package)?)?);
        return Ok(());
    }

    if markdown {
        if expand_tables {
            println!("{}", expanded_markdown_export(&package)?);
            return Ok(());
        }
        println!("{}", original_markdown_export(&package)?);
        return Ok(());
    }

    if tables {
        println!(
            "{}",
            serde_json::to_string_pretty(&table_export(&package)?)?
        );
        return Ok(());
    }

    if images {
        println!(
            "{}",
            serde_json::to_string_pretty(&image_export(&package)?)?
        );
        return Ok(());
    }

    if annotations {
        let annotations = filtered_annotation_export(annotation_export(&package)?, page, line);
        let value = if annotations.annotations.is_empty() {
            json!({
                "annotations": [],
                "message": "no annotations found"
            })
        } else {
            serde_json::to_value(&annotations)?
        };
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    if charts {
        println!(
            "{}",
            serde_json::to_string_pretty(&chart_export(&package)?)?
        );
        return Ok(());
    }

    Ok(())
}

fn filtered_annotation_export(
    mut export: AnnotationExport,
    page: Option<&str>,
    line: Option<usize>,
) -> AnnotationExport {
    if page.is_none() && line.is_none() {
        return export;
    }

    export
        .annotations
        .retain(|annotation| annotation_matches(annotation, page, line));
    export
}

fn annotation_matches(
    annotation: &AnnotationMetadata,
    page: Option<&str>,
    line: Option<usize>,
) -> bool {
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
        if line < source.start_line || line > source.end_line {
            return false;
        }
    }
    true
}
