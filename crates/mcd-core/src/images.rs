//! Image metadata parsing and validation.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    Manifest,
    assets::validate_image_asset,
    directives::ImagePlacement,
    document::{DocumentBlock, McdDocument, SourceSpan},
    errors::{Diagnostic, McdError, Result},
    manifest::{ConformanceClaim, ImageManifestEntry},
    package::McdPackage,
};

/// Parsed image metadata object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    /// Stable image id.
    pub id: String,
    /// Package asset path.
    pub asset: String,
    /// Declared asset media type.
    pub media_type: String,
    /// Image role.
    pub role: ImageRole,
    /// Optional caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Optional alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// Optional intrinsic size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_size: Option<IntrinsicSize>,
    /// Optional asset hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Optional accessibility override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<AccessibilityMetadata>,
    /// Optional declaration for meaningful visual-only content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meaningful_content: Option<MeaningfulContent>,
}

impl ImageMetadata {
    /// Parse image metadata from a package entry.
    pub fn from_package(package: &McdPackage, path: &str) -> Result<Self> {
        let bytes = package.read(path).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "image.metadata.missing",
                    format!("Declared image metadata file '{path}' is missing."),
                )
                .with_source(path.to_owned()),
            )
        })?;
        serde_json::from_slice::<Self>(bytes).map_err(McdError::from)
    }

    /// Validate image metadata and referenced asset.
    pub fn validate(
        &self,
        expected_id: &str,
        manifest: &Manifest,
        package: &McdPackage,
        source: &str,
    ) -> Result<()> {
        if self.id != expected_id {
            return Err(image_error(
                "image.id.mismatch",
                format!(
                    "Image metadata id '{}' does not match manifest image id '{}'.",
                    self.id, expected_id
                ),
                source,
            ));
        }
        if self.id.trim().is_empty() {
            return Err(image_error(
                "image.id.empty",
                "Image metadata id cannot be empty.",
                source,
            ));
        }
        validate_intrinsic_size(self.intrinsic_size.as_ref(), source)?;
        validate_role_text(self, manifest, source)?;
        validate_meaningful_content(self.meaningful_content.as_ref(), source)?;
        validate_image_asset(
            package,
            &self.asset,
            &self.media_type,
            self.hash.as_deref(),
            &manifest.assets,
            source,
        )?;
        Ok(())
    }
}

/// Supported image roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageRole {
    /// Decorative, non-semantic image.
    Decorative,
    /// Informative image.
    Informative,
    /// Diagram.
    Diagram,
    /// Photo.
    Photo,
    /// Logo.
    Logo,
    /// Prohibited rendered table role.
    RenderedTableProhibited,
    /// Prohibited rendered text role.
    RenderedTextProhibited,
}

/// Intrinsic image size metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntrinsicSize {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Unit, usually `px`.
    pub unit: String,
}

/// Accessibility metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityMetadata {
    /// Permit alt text on decorative images when explicitly justified by metadata.
    #[serde(default)]
    pub allow_decorative_alt: bool,
}

/// Declaration that an image contains meaningful information and where it is canonicalized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeaningfulContent {
    /// Image contains meaningful text.
    #[serde(default)]
    pub text: bool,
    /// Image contains meaningful numbers.
    #[serde(default)]
    pub numbers: bool,
    /// Image contains table-like data.
    #[serde(default)]
    pub table_data: bool,
    /// Markdown block or placement refs that canonicalize the content.
    #[serde(default)]
    pub markdown_refs: Vec<String>,
    /// Table ids that canonicalize the content.
    #[serde(default)]
    pub table_refs: Vec<String>,
}

/// Load and validate all manifest-declared images.
pub fn load_manifest_images(
    package: &McdPackage,
    manifest: &Manifest,
) -> Result<IndexMap<String, ImageMetadata>> {
    let mut images = IndexMap::new();
    for entry in &manifest.images {
        let metadata = load_manifest_image(package, manifest, entry)?;
        images.insert(entry.id.clone(), metadata);
    }
    Ok(images)
}

/// Resolve Markdown image anchors to loaded image metadata.
pub fn validate_image_anchors(
    document: &McdDocument,
    images: &IndexMap<String, ImageMetadata>,
) -> Result<()> {
    for block in &document.blocks {
        let DocumentBlock::ImageRef {
            placement, source, ..
        } = block
        else {
            continue;
        };
        resolve_image_placement(placement, images).ok_or_else(|| {
            image_anchor_error(
                "image.anchor.unresolved",
                "Image anchor does not resolve to a declared image metadata object.",
                document,
                *source,
            )
        })?;
    }
    Ok(())
}

fn load_manifest_image(
    package: &McdPackage,
    manifest: &Manifest,
    entry: &ImageManifestEntry,
) -> Result<ImageMetadata> {
    let metadata = ImageMetadata::from_package(package, &entry.metadata)?;
    metadata.validate(&entry.id, manifest, package, &entry.metadata)?;
    Ok(metadata)
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

fn validate_role_text(metadata: &ImageMetadata, manifest: &Manifest, source: &str) -> Result<()> {
    if manifest.conformance.contains(&ConformanceClaim::Strict)
        && matches!(
            metadata.role,
            ImageRole::RenderedTableProhibited | ImageRole::RenderedTextProhibited
        )
    {
        return Err(image_error(
            "image.role.strict.invalid",
            "Rendered table/text prohibited image roles are invalid in MCD-Strict packages.",
            source,
        ));
    }

    let alt = metadata.alt.as_deref().unwrap_or_default();
    match metadata.role {
        ImageRole::Decorative => {
            if !alt.is_empty()
                && !metadata
                    .accessibility
                    .as_ref()
                    .is_some_and(|accessibility| accessibility.allow_decorative_alt)
            {
                return Err(image_error(
                    "image.alt.decorative.nonempty",
                    "Decorative images must have empty alt text unless accessibility metadata explicitly permits it.",
                    source,
                ));
            }
        }
        ImageRole::Informative | ImageRole::Diagram | ImageRole::Photo | ImageRole::Logo => {
            if alt.trim().is_empty() {
                return Err(image_error(
                    "image.alt.missing",
                    format!("{:?} images require non-empty alt text.", metadata.role),
                    source,
                ));
            }
        }
        ImageRole::RenderedTableProhibited | ImageRole::RenderedTextProhibited => {}
    }

    if matches!(metadata.role, ImageRole::Informative | ImageRole::Diagram)
        && metadata
            .caption
            .as_deref()
            .is_none_or(|caption| caption.trim().is_empty())
    {
        return Err(image_error(
            "image.caption.missing",
            "Informative and diagram images require a caption.",
            source,
        ));
    }

    Ok(())
}

fn validate_intrinsic_size(size: Option<&IntrinsicSize>, source: &str) -> Result<()> {
    let Some(size) = size else {
        return Ok(());
    };
    if size.width == 0 || size.height == 0 {
        return Err(image_error(
            "image.intrinsic_size.invalid",
            "Intrinsic image dimensions must be greater than zero.",
            source,
        ));
    }
    if size.unit != "px" {
        return Err(image_error(
            "image.intrinsic_size.unit.unsupported",
            "Only px intrinsic image size units are supported in this alpha.",
            source,
        ));
    }
    Ok(())
}

fn validate_meaningful_content(content: Option<&MeaningfulContent>, source: &str) -> Result<()> {
    let Some(content) = content else {
        return Ok(());
    };
    let declares_meaningful_content = content.text || content.numbers || content.table_data;
    if declares_meaningful_content
        && content.markdown_refs.is_empty()
        && content.table_refs.is_empty()
    {
        return Err(image_error(
            "image.meaningful_content.unlinked",
            "Images declaring meaningful text, numbers, or table-like data must link to canonical Markdown or table references.",
            source,
        ));
    }
    Ok(())
}

fn image_anchor_error(
    code: impl Into<String>,
    message: impl Into<String>,
    document: &McdDocument,
    source: Option<SourceSpan>,
) -> McdError {
    let source = source
        .map(|span| format!("{}:{span}", document.source_path))
        .unwrap_or_else(|| document.source_path.clone());
    image_error(code, message, &source)
}

fn image_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorative_image_rejects_alt_text_without_override() {
        let metadata = ImageMetadata {
            id: "cover".to_owned(),
            asset: "assets/cover.svg".to_owned(),
            media_type: "image/svg+xml".to_owned(),
            role: ImageRole::Decorative,
            caption: None,
            alt: Some("Cover".to_owned()),
            intrinsic_size: None,
            hash: None,
            accessibility: None,
            meaningful_content: None,
        };

        let manifest = Manifest {
            format: "MCD".to_owned(),
            version: "0.1".to_owned(),
            profile: crate::manifest::McdProfile::Core,
            conformance: Vec::new(),
            entrypoint: "content/main.md".to_owned(),
            title: None,
            encoding: None,
            tables: Vec::new(),
            images: Vec::new(),
            annotations: Vec::new(),
            assets: Vec::new(),
            external_data: Vec::new(),
            provenance: None,
            layout: None,
        };

        let err = validate_role_text(&metadata, &manifest, "images/cover.image.json")
            .expect_err("invalid");
        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("image.alt.decorative.nonempty")
        );
    }

    #[test]
    fn meaningful_content_requires_canonical_refs() {
        let err = validate_meaningful_content(
            Some(&MeaningfulContent {
                text: false,
                numbers: true,
                table_data: true,
                markdown_refs: Vec::new(),
                table_refs: Vec::new(),
            }),
            "images/table.image.json",
        )
        .expect_err("invalid");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("image.meaningful_content.unlinked")
        );
    }
}
