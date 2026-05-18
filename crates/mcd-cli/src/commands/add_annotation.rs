use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use mcd_core::{
    McdPackage,
    document::SourceSpan,
    package::{MCD_MIMETYPE, validate_internal_path},
    validate::validate_package,
};
use serde_json::{Value, json};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub fn run(
    file: &Path,
    text: &str,
    page: &str,
    line: Option<usize>,
    id: Option<&str>,
) -> Result<()> {
    if text.trim().is_empty() {
        bail!("annotation text cannot be empty");
    }
    validate_internal_path(page)?;
    if line == Some(0) {
        bail!("annotation line must be 1 or greater");
    }

    let package = McdPackage::open_path(file)?;
    if !package.contains(page) {
        bail!("annotation page is not present in package: {page}");
    }

    let mut manifest = manifest_json(&package)?;
    let annotation_id = match id {
        Some(id) => validate_annotation_id(id)?,
        None => next_annotation_id(&manifest),
    };
    let metadata_path = format!("annotations/{annotation_id}.annotation.json");
    if package.contains(&metadata_path) {
        bail!("annotation metadata already exists: {metadata_path}");
    }

    append_manifest_annotation(&mut manifest, &annotation_id, &metadata_path)?;
    let annotation = annotation_json(&annotation_id, text, page, line);

    let mut entries = package
        .entry_paths()
        .into_iter()
        .filter(|entry| *entry != "manifest.json")
        .map(|entry| Ok((entry.to_owned(), package.read(entry)?.to_vec())))
        .collect::<std::result::Result<Vec<_>, mcd_core::McdError>>()?;
    entries.push((
        "manifest.json".to_owned(),
        serde_json::to_vec_pretty(&manifest)?,
    ));
    entries.push((
        metadata_path.clone(),
        serde_json::to_vec_pretty(&annotation)?,
    ));

    write_package(file, entries)?;

    let updated = McdPackage::open_path(file)?;
    validate_package(&updated)?;
    println!("{annotation_id}");
    Ok(())
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
        bail!(
            "annotation id must match [A-Za-z0-9][A-Za-z0-9_.-]*; got '{}'",
            id
        );
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
