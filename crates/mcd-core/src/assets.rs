//! Package asset validation helpers.

use sha2::{Digest, Sha256};

use crate::{
    errors::{Diagnostic, McdError, Result},
    manifest::AssetManifestEntry,
    package::{McdPackage, validate_internal_path},
};

/// Return true when an asset path is allowed by the default asset directory or manifest entries.
#[must_use]
pub fn asset_path_allowed(path: &str, declared_assets: &[AssetManifestEntry]) -> bool {
    path.starts_with("assets/")
        || declared_assets
            .iter()
            .any(|asset| path_allowed_by(path, asset))
}

/// Validate asset path, existence, media type, hash, and SVG safety.
pub fn validate_image_asset(
    package: &McdPackage,
    path: &str,
    declared_media_type: &str,
    hash: Option<&str>,
    declared_assets: &[AssetManifestEntry],
    source: &str,
) -> Result<()> {
    validate_internal_path(path).map_err(|_| {
        McdError::from_diagnostic(
            Diagnostic::error(
                "asset.path.invalid",
                format!("Image asset path '{path}' is not a safe package path."),
            )
            .with_source(source.to_owned()),
        )
    })?;

    if !asset_path_allowed(path, declared_assets) {
        return Err(asset_error(
            "asset.path.disallowed",
            format!("Image asset '{path}' must be inside assets/ or a declared asset path."),
            source,
        ));
    }

    let bytes = package.read(path).map_err(|_| {
        McdError::from_diagnostic(
            Diagnostic::error(
                "asset.missing",
                format!("Referenced image asset '{path}' is missing."),
            )
            .with_source(path.to_owned()),
        )
    })?;

    let detected = detect_media_type(path);
    if detected != declared_media_type {
        return Err(asset_error(
            "asset.media_type.mismatch",
            format!(
                "Asset '{}' declares media type '{}', but path detection found '{}'.",
                path, declared_media_type, detected
            ),
            source,
        ));
    }

    if !declared_media_type.starts_with("image/") {
        return Err(asset_error(
            "asset.media_type.unsupported",
            format!("Asset '{path}' must declare an image media type."),
            source,
        ));
    }

    if let Some(hash) = hash {
        validate_sha256_hash(bytes, hash, source)?;
    }

    if declared_media_type == "image/svg+xml" {
        validate_svg_safety(bytes, path)?;
    }

    Ok(())
}

/// Detect media type from the asset path.
#[must_use]
pub fn detect_media_type(path: &str) -> String {
    if path.to_ascii_lowercase().ends_with(".svg") {
        "image/svg+xml".to_owned()
    } else {
        mime_guess::from_path(path).first().map_or_else(
            || "application/octet-stream".to_owned(),
            |mime| mime.to_string(),
        )
    }
}

/// Compute a stable `sha256:<hex>` hash string.
#[must_use]
pub fn sha256_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn validate_sha256_hash(bytes: &[u8], declared_hash: &str, source: &str) -> Result<()> {
    if !declared_hash.starts_with("sha256:") || declared_hash.len() != "sha256:".len() + 64 {
        return Err(asset_error(
            "asset.hash.invalid",
            "Image asset hash must use sha256:<64 lowercase hex characters>.",
            source,
        ));
    }
    if !declared_hash["sha256:".len()..]
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(asset_error(
            "asset.hash.invalid",
            "Image asset hash must use hexadecimal characters.",
            source,
        ));
    }

    let actual = sha256_hash(bytes);
    if actual != declared_hash {
        return Err(asset_error(
            "asset.hash.mismatch",
            "Image asset hash does not match package bytes.",
            source,
        ));
    }

    Ok(())
}

fn validate_svg_safety(bytes: &[u8], path: &str) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        McdError::from_diagnostic(
            Diagnostic::error("asset.svg.utf8", "SVG asset is not valid UTF-8.")
                .with_source(path.to_owned()),
        )
    })?;
    let doc = roxmltree::Document::parse(text).map_err(|err| {
        McdError::from_diagnostic(
            Diagnostic::error(
                "asset.svg.invalid",
                format!("SVG asset is not valid XML: {err}."),
            )
            .with_source(path.to_owned()),
        )
    })?;

    for node in doc.descendants().filter(roxmltree::Node::is_element) {
        let tag_name = node.tag_name().name();
        if matches!(
            tag_name,
            "script" | "foreignObject" | "animate" | "animateMotion" | "animateTransform" | "set"
        ) {
            return Err(asset_error(
                "security.svg.active_content",
                format!("SVG asset contains disallowed <{tag_name}> content."),
                path,
            ));
        }

        for attribute in node.attributes() {
            let name = attribute.name();
            let value = attribute.value().trim();
            if name == "xmlns" || name.starts_with("xmlns:") {
                continue;
            }
            if name.starts_with("on") {
                return Err(asset_error(
                    "security.svg.event_handler",
                    format!("SVG asset contains disallowed event handler attribute '{name}'."),
                    path,
                ));
            }
            if is_external_reference(value) {
                return Err(asset_error(
                    "security.svg.external_reference",
                    format!("SVG asset contains disallowed external reference '{value}'."),
                    path,
                ));
            }
        }
    }

    Ok(())
}

fn is_external_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("//")
        || lower.contains("javascript:")
        || lower.contains("data:")
}

fn path_allowed_by(path: &str, asset: &AssetManifestEntry) -> bool {
    let declared = asset.path.trim_end_matches('/');
    path == declared || path.starts_with(&format!("{declared}/"))
}

fn asset_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_svg_media_type() {
        assert_eq!(detect_media_type("assets/process.svg"), "image/svg+xml");
    }

    #[test]
    fn hashes_asset_bytes() {
        assert_eq!(
            sha256_hash(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
