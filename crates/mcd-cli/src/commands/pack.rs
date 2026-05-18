use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use mcd_core::package::{MCD_MIMETYPE, validate_internal_path};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub fn run(directory: &Path, output: &Path) -> Result<()> {
    if !directory.is_dir() {
        bail!("pack source must be a directory: {}", directory.display());
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
        std::io::copy(&mut input, &mut writer)?;
        files.retain(|path| path != &mimetype);
    } else {
        use std::io::Write;
        writer.write_all(MCD_MIMETYPE.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    for path in files {
        let internal_path = internal_path(directory, &path)?;
        writer.start_file(&internal_path, deflated)?;
        let mut input = File::open(&path)?;
        std::io::copy(&mut input, &mut writer)?;
    }

    writer.finish()?;
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
