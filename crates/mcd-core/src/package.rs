//! Safe package archive reading.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Cursor, Read, Seek},
    path::Path,
};

use camino::Utf8PathBuf;
use zip::ZipArchive;

use crate::{
    errors::{Diagnostic, McdError, Result},
    manifest::Manifest,
};

/// Required MCD package media type.
pub const MCD_MIMETYPE: &str = "application/vnd.mcd+zip";

const MAX_FILE_COUNT: usize = 10_000;
const MAX_SINGLE_FILE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_DECOMPRESSED_SIZE: u64 = 512 * 1024 * 1024;

/// An opened MCD package with validated internal paths.
#[derive(Debug, Clone)]
pub struct McdPackage {
    entries: HashMap<String, Vec<u8>>,
}

impl McdPackage {
    /// Open a package from a filesystem path.
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_reader(file)
    }

    /// Open a package from in-memory bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::from_reader(Cursor::new(bytes))
    }

    /// Open a package from any readable and seekable ZIP stream.
    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let mut archive = ZipArchive::new(reader)?;
        if archive.len() > MAX_FILE_COUNT {
            return Err(McdError::from_diagnostic(Diagnostic::error(
                "package.file_count.exceeded",
                format!("Package contains more than {MAX_FILE_COUNT} entries."),
            )));
        }

        let mut entries = HashMap::new();
        let mut seen = HashSet::new();
        let mut total_size = 0_u64;

        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }

            let name = file.name().to_owned();
            let normalized = validate_internal_path(&name)?;

            if !seen.insert(normalized.clone()) {
                return Err(McdError::from_diagnostic(
                    Diagnostic::error(
                        "security.path.duplicate",
                        format!("Duplicate normalized package path '{normalized}'."),
                    )
                    .with_source(name),
                ));
            }

            let size = file.size();
            if size > MAX_SINGLE_FILE_SIZE {
                return Err(McdError::from_diagnostic(
                    Diagnostic::error(
                        "package.file_size.exceeded",
                        format!("Package entry '{normalized}' exceeds the single-file size limit."),
                    )
                    .with_source(normalized),
                ));
            }

            total_size = total_size.checked_add(size).ok_or_else(|| {
                McdError::from_diagnostic(Diagnostic::error(
                    "package.total_size.overflow",
                    "Package decompressed size overflowed.",
                ))
            })?;
            if total_size > MAX_TOTAL_DECOMPRESSED_SIZE {
                return Err(McdError::from_diagnostic(Diagnostic::error(
                    "package.total_size.exceeded",
                    format!(
                        "Package exceeds the total decompressed size limit of {MAX_TOTAL_DECOMPRESSED_SIZE} bytes."
                    ),
                )));
            }

            let mut bytes = Vec::with_capacity(size.try_into().unwrap_or(0));
            file.read_to_end(&mut bytes)?;
            entries.insert(normalized, bytes);
        }

        let package = Self { entries };
        package.validate_mimetype()?;
        Ok(package)
    }

    /// Return sorted package entry paths.
    #[must_use]
    pub fn entry_paths(&self) -> Vec<&str> {
        let mut paths = self.entries.keys().map(String::as_str).collect::<Vec<_>>();
        paths.sort_unstable();
        paths
    }

    /// Check if the package has a path.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// Read package bytes for an internal path.
    pub fn read(&self, path: &str) -> Result<&[u8]> {
        let normalized = validate_internal_path(path)?;
        self.entries
            .get(&normalized)
            .map(Vec::as_slice)
            .ok_or_else(|| {
                McdError::from_diagnostic(
                    Diagnostic::error(
                        "package.entry.missing",
                        format!("Package entry '{normalized}' is missing."),
                    )
                    .with_source(normalized),
                )
            })
    }

    /// Read a package entry as UTF-8 text.
    pub fn read_to_string(&self, path: &str) -> Result<String> {
        String::from_utf8(self.read(path)?.to_vec()).map_err(McdError::from)
    }

    /// Parse the root manifest.
    pub fn manifest(&self) -> Result<Manifest> {
        let bytes = self.read("manifest.json").map_err(|err| {
            err.diagnostic().map_or(err, |_| {
                McdError::from_diagnostic(
                    Diagnostic::error("manifest.missing", "Package is missing manifest.json.")
                        .with_source("manifest.json"),
                )
            })
        })?;
        Manifest::from_slice(bytes)
    }

    /// Validate the root `mimetype` entry.
    pub fn validate_mimetype(&self) -> Result<()> {
        let bytes = self.entries.get("mimetype").ok_or_else(|| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "package.mimetype.missing",
                    "Package is missing root mimetype.",
                )
                .with_source("mimetype"),
            )
        })?;
        let mimetype = std::str::from_utf8(bytes).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "package.mimetype.utf8",
                    "Package mimetype is not valid UTF-8.",
                )
                .with_source("mimetype"),
            )
        })?;
        if mimetype.trim_end_matches(['\r', '\n']) != MCD_MIMETYPE {
            return Err(McdError::from_diagnostic(
                Diagnostic::error(
                    "package.mimetype.invalid",
                    format!("Package mimetype must be '{MCD_MIMETYPE}'."),
                )
                .with_source("mimetype"),
            ));
        }
        Ok(())
    }
}

/// Validate and normalize a package-internal path.
pub fn validate_internal_path(path: &str) -> Result<String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains(':')
        || path.contains('\0')
    {
        return Err(invalid_path(path));
    }

    let mut normalized = Utf8PathBuf::new();
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(invalid_path(path));
        }
        normalized.push(component);
    }

    let normalized = normalized.as_str().replace('\\', "/");
    if normalized != path {
        return Err(invalid_path(path));
    }

    Ok(normalized)
}

fn invalid_path(path: &str) -> McdError {
    McdError::from_diagnostic(
        Diagnostic::error(
            "security.path.invalid",
            format!("Package path '{path}' is not a safe relative path."),
        )
        .with_source(path.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_safe_paths() {
        assert_eq!(
            validate_internal_path("content/main.md").expect("valid path"),
            "content/main.md"
        );
    }

    #[test]
    fn rejects_traversal() {
        let err = validate_internal_path("content/../manifest.json").expect_err("invalid path");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("security.path.invalid")
        );
    }

    #[test]
    fn rejects_windows_separator() {
        let err = validate_internal_path("content\\main.md").expect_err("invalid path");
        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("security.path.invalid")
        );
    }
}
