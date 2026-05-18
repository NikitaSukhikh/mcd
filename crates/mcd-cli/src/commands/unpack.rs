use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use mcd_core::McdPackage;

pub fn run(file: &Path, output: &Path) -> Result<()> {
    if output.exists() && !output.is_dir() {
        bail!("unpack output must be a directory: {}", output.display());
    }

    let package = McdPackage::open_path(file)?;
    fs::create_dir_all(output).with_context(|| format!("create {}", output.display()))?;

    for entry in package.entry_paths() {
        let target = output_path(output, entry);
        if target.exists() {
            bail!("refusing to overwrite existing file: {}", target.display());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&target, package.read(entry)?)
            .with_context(|| format!("write {}", target.display()))?;
    }

    Ok(())
}

fn output_path(output: &Path, entry: &str) -> PathBuf {
    entry
        .split('/')
        .fold(output.to_path_buf(), |path, component| path.join(component))
}
