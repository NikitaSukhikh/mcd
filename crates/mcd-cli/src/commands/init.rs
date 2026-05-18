use std::{fs, path::Path};

use anyhow::Result;

pub fn run(directory: &Path) -> Result<()> {
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
    Ok(())
}
