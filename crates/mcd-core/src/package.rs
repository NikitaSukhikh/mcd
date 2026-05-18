//! Safe package archive reading.

use std::{
    collections::{HashMap, HashSet},
    fs,
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
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Open a package from in-memory bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match Self::from_reader(Cursor::new(bytes)) {
            Ok(package) => Ok(package),
            Err(err) if is_plain_markdown_candidate(bytes) => {
                let markdown = std::str::from_utf8(bytes).map_err(|_| err)?;
                Ok(Self::from_markdown(markdown))
            }
            Err(err) => Err(err),
        }
    }

    /// Build a minimal package from a standalone Markdown document.
    #[must_use]
    pub fn from_markdown(markdown: &str) -> Self {
        let mut entries = HashMap::new();
        entries.insert(
            "mimetype".to_owned(),
            format!("{MCD_MIMETYPE}\n").into_bytes(),
        );
        entries.insert(
            "manifest.json".to_owned(),
            br#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#
                .to_vec(),
        );
        entries.insert("content/main.md".to_owned(), markdown.as_bytes().to_vec());
        Self { entries }
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

            let duplicate_key = normalized.to_ascii_lowercase();
            if !seen.insert(duplicate_key) {
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
            if err.diagnostic().is_some() {
                McdError::from_diagnostic(
                    Diagnostic::error("manifest.missing", "Package is missing manifest.json.")
                        .with_source("manifest.json"),
                )
            } else {
                err
            }
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

fn is_plain_markdown_candidate(bytes: &[u8]) -> bool {
    !bytes.starts_with(b"PK") && std::str::from_utf8(bytes).is_ok()
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
    use proptest::prelude::*;
    use std::io::Write;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

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

    #[test]
    fn opens_valid_minimal_package() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#,
            ),
            ("content/main.md", "# Minimal\n"),
        ]))
        .expect("package opens");

        assert_eq!(
            package.manifest().expect("manifest").entrypoint,
            "content/main.md"
        );
    }

    #[test]
    fn opens_plain_markdown_as_minimal_package() {
        let markdown = "# Plain Markdown\n\nThis file was renamed to .mcd.\n";
        let package = McdPackage::from_bytes(markdown.as_bytes()).expect("markdown opens");

        assert_eq!(
            package.entry_paths(),
            vec!["content/main.md", "manifest.json", "mimetype"]
        );
        assert_eq!(
            package.manifest().expect("manifest").entrypoint,
            "content/main.md"
        );
        assert_eq!(
            package
                .read_to_string("content/main.md")
                .expect("entrypoint markdown"),
            markdown
        );
    }

    #[test]
    fn missing_mimetype_fails_with_diagnostic() {
        let err = McdPackage::from_bytes(&zip_bytes(&[(
            "manifest.json",
            r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#,
        )]))
        .expect_err("missing mimetype should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("package.mimetype.missing")
        );
    }

    #[test]
    fn bad_mimetype_fails_with_diagnostic() {
        let err = McdPackage::from_bytes(&zip_bytes(&[("mimetype", "text/plain")]))
            .expect_err("bad mimetype should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("package.mimetype.invalid")
        );
    }

    #[test]
    fn missing_manifest_fails_with_diagnostic() {
        let package =
            McdPackage::from_bytes(&zip_bytes(&[("mimetype", MCD_MIMETYPE)])).expect("opens");
        let err = package
            .manifest()
            .expect_err("missing manifest should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("manifest.missing")
        );
    }

    #[test]
    fn path_traversal_fixture_fails() {
        let err = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", MCD_MIMETYPE),
            ("../manifest.json", "{}"),
        ]))
        .expect_err("traversal should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("security.path.invalid")
        );
    }

    #[test]
    fn duplicate_normalized_path_fails() {
        let err = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", MCD_MIMETYPE),
            ("manifest.json", "{}"),
            ("Manifest.json", "{}"),
        ]))
        .expect_err("duplicate path should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("security.path.duplicate")
        );
    }

    proptest! {
        #[test]
        fn validates_generated_safe_relative_paths(segments in prop::collection::vec("[A-Za-z0-9_-]{1,12}", 1..5)) {
            let path = segments.join("/");
            let normalized = validate_internal_path(&path).expect("safe relative path should validate");
            prop_assert_eq!(normalized, path);
        }

        #[test]
        fn rejects_generated_traversal_paths(prefix in "[A-Za-z0-9_-]{1,12}", suffix in "[A-Za-z0-9_-]{1,12}") {
            let path = format!("{prefix}/../{suffix}");
            let err = validate_internal_path(&path).expect_err("traversal path should fail");
            prop_assert_eq!(
                err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
                Some("security.path.invalid")
            );
        }
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
