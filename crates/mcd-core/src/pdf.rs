//! Simple PDF-to-MCD conversion.

use std::io::{Cursor, Write};

use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    errors::{Diagnostic, McdError},
    manifest::{AssetManifestEntry, ConformanceClaim, LayoutManifestEntry, Manifest, McdProfile},
    package::MCD_MIMETYPE,
};

/// Options for converting a PDF into a minimal MCD package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfConversionOptions {
    /// Optional document title for the MCD manifest and Markdown heading.
    pub title: Option<String>,
    /// Optional original file name used for the embedded PDF asset path.
    pub source_filename: Option<String>,
}

/// Convert PDF bytes into a minimal MCD archive.
///
/// The converter extracts text into `content/main.md` and embeds the original
/// PDF as `assets/<source_filename>`. It does not attempt OCR, table recovery,
/// or layout reconstruction.
pub fn pdf_to_mcd_bytes(pdf: &[u8], options: PdfConversionOptions) -> crate::Result<Vec<u8>> {
    if !pdf.starts_with(b"%PDF-") {
        return Err(McdError::from_diagnostic(Diagnostic::error(
            "pdf.signature.invalid",
            "Input does not look like a PDF file.",
        )));
    }

    let pages = extract_pdf_pages(pdf)?;
    let asset_path = format!(
        "assets/{}",
        sanitize_pdf_filename(options.source_filename.as_deref())
    );
    let title = options
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| title_from_asset_path(&asset_path));
    let markdown = markdown_from_pdf_pages(&title, &asset_path, &pages);
    let manifest = Manifest {
        format: "MCD".to_owned(),
        version: "0.1".to_owned(),
        profile: McdProfile::Core,
        conformance: vec![ConformanceClaim::Core],
        entrypoint: "content/main.md".to_owned(),
        title: Some(title),
        encoding: Some("utf-8".to_owned()),
        tables: Vec::new(),
        images: Vec::new(),
        annotations: Vec::new(),
        assets: vec![AssetManifestEntry {
            id: Some("source-pdf".to_owned()),
            path: asset_path.clone(),
        }],
        external_data: Vec::new(),
        layout: None::<LayoutManifestEntry>,
    };

    let manifest_json = serde_json::to_vec_pretty(&manifest)?;
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    archive.start_file("mimetype", stored)?;
    archive.write_all(MCD_MIMETYPE.as_bytes())?;
    archive.write_all(b"\n")?;
    archive.start_file("manifest.json", deflated)?;
    archive.write_all(&manifest_json)?;
    archive.start_file("content/main.md", deflated)?;
    archive.write_all(markdown.as_bytes())?;
    archive.start_file(asset_path, deflated)?;
    archive.write_all(pdf)?;

    Ok(archive.finish()?.into_inner())
}

fn extract_pdf_pages(pdf: &[u8]) -> crate::Result<Vec<String>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match pdf_extract::extract_text_from_mem_by_pages(pdf) {
            Ok(pages) => Ok(pages),
            Err(err) => {
                let fallback_pages = fallback_extract_literal_text(pdf);
                if fallback_pages.is_empty() {
                    Err(pdf_error(err))
                } else {
                    Ok(fallback_pages)
                }
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(fallback_extract_literal_text(pdf))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pdf_error(err: pdf_extract::OutputError) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(
        "pdf.text.extract.failed",
        format!("Failed to extract text from PDF: {err}"),
    ))
}

fn fallback_extract_literal_text(pdf: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut index = 0;
    while index < pdf.len() {
        if pdf[index] != b'(' {
            index += 1;
            continue;
        }

        if let Some((value, end)) = parse_pdf_literal_string(pdf, index + 1) {
            if looks_like_text_showing_operator(pdf, end) && !value.trim().is_empty() {
                strings.push(value);
            }
            index = end + 1;
        } else {
            index += 1;
        }
    }

    if strings.is_empty() {
        Vec::new()
    } else {
        vec![strings.join("\n")]
    }
}

fn parse_pdf_literal_string(pdf: &[u8], mut index: usize) -> Option<(String, usize)> {
    let mut value = Vec::new();
    let mut depth = 1_u32;
    while index < pdf.len() {
        let byte = pdf[index];
        match byte {
            b'\\' => {
                index += 1;
                if index >= pdf.len() {
                    return None;
                }
                match pdf[index] {
                    b'n' => value.push(b'\n'),
                    b'r' => value.push(b'\r'),
                    b't' => value.push(b'\t'),
                    b'b' => value.push(0x08),
                    b'f' => value.push(0x0c),
                    b'\n' => {}
                    b'\r' => {
                        if pdf.get(index + 1) == Some(&b'\n') {
                            index += 1;
                        }
                    }
                    escaped => value.push(escaped),
                }
            }
            b'(' => {
                depth += 1;
                value.push(byte);
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((String::from_utf8_lossy(&value).into_owned(), index));
                }
                value.push(byte);
            }
            _ => value.push(byte),
        }
        index += 1;
    }
    None
}

fn looks_like_text_showing_operator(pdf: &[u8], end: usize) -> bool {
    let tail_start = end.saturating_add(1);
    let tail_end = tail_start.saturating_add(32).min(pdf.len());
    let tail = &pdf[tail_start..tail_end];
    let tail = trim_ascii_start(tail);
    tail.starts_with(b"Tj")
        || tail.starts_with(b"'")
        || tail.starts_with(b"\"")
        || tail.windows(2).take(16).any(|window| window == b"TJ")
}

fn trim_ascii_start(mut bytes: &[u8]) -> &[u8] {
    while let Some((first, rest)) = bytes.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        bytes = rest;
    }
    bytes
}

fn markdown_from_pdf_pages(title: &str, asset_path: &str, pages: &[String]) -> String {
    let mut markdown = String::new();
    markdown.push_str("# ");
    markdown.push_str(&escape_heading(title));
    markdown.push_str("\n\n");
    markdown.push_str("Source PDF asset: `");
    markdown.push_str(asset_path);
    markdown.push_str("`.\n");

    if pages.is_empty() || pages.iter().all(|page| page.trim().is_empty()) {
        markdown.push_str("\n_No extractable text was found in the PDF._\n");
        return markdown;
    }

    for (index, page) in pages.iter().enumerate() {
        let text = normalize_pdf_text(page);
        if text.trim().is_empty() {
            continue;
        }
        markdown.push_str("\n\n## Page ");
        markdown.push_str(&(index + 1).to_string());
        markdown.push_str("\n\n");
        markdown.push_str(&text);
    }
    markdown.push('\n');
    markdown
}

fn normalize_pdf_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn sanitize_pdf_filename(source_filename: Option<&str>) -> String {
    let source_filename = source_filename
        .and_then(|path| {
            path.rsplit(['/', '\\'])
                .find(|part| !part.trim().is_empty())
        })
        .unwrap_or("source.pdf");
    let mut sanitized = source_filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    while sanitized.contains("..") {
        sanitized = sanitized.replace("..", ".");
    }
    sanitized = sanitized.trim_matches('.').to_owned();
    if sanitized.is_empty() {
        sanitized = "source.pdf".to_owned();
    }
    if !sanitized.to_ascii_lowercase().ends_with(".pdf") {
        sanitized.push_str(".pdf");
    }
    sanitized
}

fn title_from_asset_path(asset_path: &str) -> String {
    let file_name = asset_path.rsplit('/').next().unwrap_or("source.pdf");
    let stem = file_name
        .strip_suffix(".pdf")
        .or_else(|| file_name.strip_suffix(".PDF"))
        .unwrap_or(file_name);
    let title = stem.replace(['_', '-'], " ");
    if title.trim().is_empty() {
        "Converted PDF".to_owned()
    } else {
        title.trim().to_owned()
    }
}

fn escape_heading(value: &str) -> String {
    value.replace('\n', " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pdf_input() {
        let err = pdf_to_mcd_bytes(b"not a pdf", PdfConversionOptions::default())
            .expect_err("non-pdf should fail");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("pdf.signature.invalid")
        );
    }

    #[test]
    fn sanitizes_asset_file_names() {
        assert_eq!(
            sanitize_pdf_filename(Some(r"..\Quarterly Report 2026.pdf")),
            "Quarterly_Report_2026.pdf"
        );
        assert_eq!(sanitize_pdf_filename(Some("report")), "report.pdf");
    }

    #[test]
    fn renders_empty_pdf_text_as_valid_markdown() {
        let markdown = markdown_from_pdf_pages("Report", "assets/report.pdf", &[]);

        assert!(markdown.contains("# Report"));
        assert!(markdown.contains("_No extractable text was found"));
    }

    #[test]
    fn converts_simple_pdf_to_valid_mcd_package() {
        let pdf = minimal_pdf("Hello from PDF");
        let mcd = pdf_to_mcd_bytes(
            &pdf,
            PdfConversionOptions {
                title: Some("PDF Import".to_owned()),
                source_filename: Some("import.pdf".to_owned()),
            },
        )
        .expect("pdf converts");
        let package = crate::McdPackage::from_bytes(&mcd).expect("mcd opens");
        crate::validate::validate_package(&package).expect("mcd validates");
        let markdown = package
            .read_to_string("content/main.md")
            .expect("markdown exists");

        assert!(markdown.contains("# PDF Import"));
        assert!(markdown.contains("Hello from PDF"));
        assert!(package.contains("assets/import.pdf"));
    }

    fn minimal_pdf(text: &str) -> Vec<u8> {
        let escaped = text
            .replace('\\', r"\\")
            .replace('(', r"\(")
            .replace(')', r"\)");
        let content = format!("BT /F1 24 Tf 100 700 Td ({escaped}) Tj ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
        ];
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(bytes.len());
            bytes
                .extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
        }
        let xref_offset = bytes.len();
        bytes.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        bytes.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                objects.len() + 1,
                xref_offset
            )
            .as_bytes(),
        );
        bytes
    }
}
