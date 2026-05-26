//! Package-level provenance metadata parsing and validation.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    Manifest, McdPackage,
    errors::{Diagnostic, McdError, Result},
    manifest::{is_media_type, is_sha256, is_supported_external_uri, is_valid_id},
    package::validate_internal_path,
};

/// Parsed package-level provenance metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceMetadata {
    /// Source documents or datasets used to produce the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ProvenanceSource>,
    /// People, organizations, software, or agents involved in provenance events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<ProvenanceActor>,
    /// Extraction, conversion, rendering, validation, or generation tools.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ProvenanceTool>,
    /// Generated package assets and the sources/tools that produced them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated_assets: Vec<GeneratedAsset>,
    /// Ordered or unordered provenance activities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activities: Vec<ProvenanceActivity>,
}

impl ProvenanceMetadata {
    /// Parse provenance metadata from a package entry.
    pub fn from_package(package: &McdPackage, path: &str) -> Result<Self> {
        let bytes = package.read(path).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "provenance.metadata.missing",
                    format!("Declared provenance metadata file '{path}' is missing."),
                )
                .with_source(path.to_owned()),
            )
        })?;
        serde_json::from_slice::<Self>(bytes).map_err(McdError::from)
    }

    /// Validate provenance metadata against the package and manifest.
    pub fn validate(&self, manifest: &Manifest, package: &McdPackage, source: &str) -> Result<()> {
        let indexes = ProvenanceIndexes::new(self, manifest, source)?;

        for item in &self.sources {
            item.validate(package, source)?;
        }
        for item in &self.actors {
            item.validate(source)?;
        }
        for item in &self.tools {
            item.validate(source)?;
        }
        for item in &self.generated_assets {
            item.validate(package, &indexes, source)?;
        }
        for item in &self.activities {
            item.validate(&indexes, source)?;
        }

        Ok(())
    }
}

/// A source document, dataset, or external resource used to produce package content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceSource {
    /// Stable source id.
    pub id: String,
    /// Optional packaged source path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional original or external URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Optional media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Optional integrity hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Optional expected size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Optional display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional source creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Optional source retrieval timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
}

impl ProvenanceSource {
    fn validate(&self, package: &McdPackage, source: &str) -> Result<()> {
        validate_id("provenance.source.id.invalid", &self.id, source)?;
        if self.path.is_none() && self.uri.is_none() {
            return Err(provenance_error(
                "provenance.source.location.missing",
                format!("Provenance source '{}' must declare path or uri.", self.id),
                source,
            ));
        }
        if let Some(path) = &self.path {
            validate_package_path("provenance.source.path.invalid", path, source)?;
            if !package.contains(path) {
                return Err(provenance_error(
                    "provenance.source.path.missing",
                    format!(
                        "Provenance source '{}' references missing package path '{}'.",
                        self.id, path
                    ),
                    source,
                ));
            }
        }
        if let Some(uri) = &self.uri {
            validate_uri("provenance.source.uri.invalid", uri, source)?;
        }
        validate_optional_media_type(
            &self.media_type,
            "provenance.source.media_type.invalid",
            source,
        )?;
        validate_optional_hash(&self.hash, "provenance.source.hash.invalid", source)?;
        validate_optional_text(&self.title, "provenance.source.title.empty", source)?;
        validate_optional_timestamp(
            &self.created_at,
            "provenance.source.created_at.invalid",
            source,
        )?;
        validate_optional_timestamp(
            &self.retrieved_at,
            "provenance.source.retrieved_at.invalid",
            source,
        )?;
        Ok(())
    }
}

/// Actor involved in a provenance activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceActor {
    /// Stable actor id.
    pub id: String,
    /// Actor type.
    pub kind: ActorKind,
    /// Optional actor display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional actor URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl ProvenanceActor {
    fn validate(&self, source: &str) -> Result<()> {
        validate_id("provenance.actor.id.invalid", &self.id, source)?;
        validate_optional_text(&self.name, "provenance.actor.name.empty", source)?;
        if let Some(uri) = &self.uri {
            validate_uri("provenance.actor.uri.invalid", uri, source)?;
        }
        Ok(())
    }
}

/// Supported provenance actor kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// A person.
    Person,
    /// An organization.
    Organization,
    /// A software process.
    Software,
    /// An autonomous or assisted agent.
    Agent,
}

/// Tool used in extraction, conversion, validation, rendering, or generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceTool {
    /// Stable tool id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Optional version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional tool URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Optional tool artifact hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

impl ProvenanceTool {
    fn validate(&self, source: &str) -> Result<()> {
        validate_id("provenance.tool.id.invalid", &self.id, source)?;
        if self.name.trim().is_empty() {
            return Err(provenance_error(
                "provenance.tool.name.empty",
                "Provenance tool name cannot be empty.",
                source,
            ));
        }
        validate_optional_text(&self.version, "provenance.tool.version.empty", source)?;
        if let Some(uri) = &self.uri {
            validate_uri("provenance.tool.uri.invalid", uri, source)?;
        }
        validate_optional_hash(&self.hash, "provenance.tool.hash.invalid", source)?;
        Ok(())
    }
}

/// Generated package asset with direct provenance links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedAsset {
    /// Stable generated asset id.
    pub id: String,
    /// Generated package path.
    pub path: String,
    /// Optional media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Optional generated asset hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Optional creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Source ids used to produce this asset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Tool ids used to produce this asset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_refs: Vec<String>,
    /// Actor ids responsible for this asset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actor_refs: Vec<String>,
}

impl GeneratedAsset {
    fn validate(
        &self,
        package: &McdPackage,
        indexes: &ProvenanceIndexes,
        source: &str,
    ) -> Result<()> {
        validate_id("provenance.generated_asset.id.invalid", &self.id, source)?;
        validate_package_path(
            "provenance.generated_asset.path.invalid",
            &self.path,
            source,
        )?;
        if !package.contains(&self.path) {
            return Err(provenance_error(
                "provenance.generated_asset.path.missing",
                format!(
                    "Generated asset '{}' references missing package path '{}'.",
                    self.id, self.path
                ),
                source,
            ));
        }
        validate_optional_media_type(
            &self.media_type,
            "provenance.generated_asset.media_type.invalid",
            source,
        )?;
        validate_optional_hash(
            &self.hash,
            "provenance.generated_asset.hash.invalid",
            source,
        )?;
        validate_optional_timestamp(
            &self.created_at,
            "provenance.generated_asset.created_at.invalid",
            source,
        )?;
        validate_refs(
            "provenance.generated_asset.source_ref.unresolved",
            &self.source_refs,
            &indexes.sources,
            "source",
            source,
        )?;
        validate_refs(
            "provenance.generated_asset.tool_ref.unresolved",
            &self.tool_refs,
            &indexes.tools,
            "tool",
            source,
        )?;
        validate_refs(
            "provenance.generated_asset.actor_ref.unresolved",
            &self.actor_refs,
            &indexes.actors,
            "actor",
            source,
        )?;
        Ok(())
    }
}

/// Provenance activity linking actors, tools, inputs, and outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceActivity {
    /// Stable activity id.
    pub id: String,
    /// Activity kind.
    pub kind: ActivityKind,
    /// Optional activity start timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// Optional activity end timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Actor ids involved in the activity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actor_refs: Vec<String>,
    /// Tool ids used by the activity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_refs: Vec<String>,
    /// Source ids used by the activity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Generic input references for package paths or future object kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_refs: Vec<String>,
    /// Generic output references for package paths or future object kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_refs: Vec<String>,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ProvenanceActivity {
    fn validate(&self, indexes: &ProvenanceIndexes, source: &str) -> Result<()> {
        validate_id("provenance.activity.id.invalid", &self.id, source)?;
        let started_at = validate_optional_timestamp(
            &self.started_at,
            "provenance.activity.started_at.invalid",
            source,
        )?;
        let ended_at = validate_optional_timestamp(
            &self.ended_at,
            "provenance.activity.ended_at.invalid",
            source,
        )?;
        if let (Some(started_at), Some(ended_at)) = (started_at, ended_at)
            && ended_at < started_at
        {
            return Err(provenance_error(
                "provenance.activity.time_order.invalid",
                format!("Provenance activity '{}' ends before it starts.", self.id),
                source,
            ));
        }
        validate_refs(
            "provenance.activity.actor_ref.unresolved",
            &self.actor_refs,
            &indexes.actors,
            "actor",
            source,
        )?;
        validate_refs(
            "provenance.activity.tool_ref.unresolved",
            &self.tool_refs,
            &indexes.tools,
            "tool",
            source,
        )?;
        validate_refs(
            "provenance.activity.source_ref.unresolved",
            &self.source_refs,
            &indexes.sources,
            "source",
            source,
        )?;
        validate_generic_refs(
            "provenance.activity.input_ref.invalid",
            &self.input_refs,
            indexes,
            source,
        )?;
        validate_generic_refs(
            "provenance.activity.output_ref.invalid",
            &self.output_refs,
            indexes,
            source,
        )?;
        validate_optional_text(
            &self.description,
            "provenance.activity.description.empty",
            source,
        )?;
        Ok(())
    }
}

/// Supported provenance activity kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// Human or system authored source content.
    Authored,
    /// Source material was imported.
    Imported,
    /// Content was extracted from source material.
    Extracted,
    /// Content was transformed.
    Transformed,
    /// New content or assets were generated.
    Generated,
    /// Package content was validated.
    Validated,
    /// Package content was rendered.
    Rendered,
    /// Package content was reviewed.
    Reviewed,
}

/// Load and validate manifest-declared provenance metadata.
pub fn load_manifest_provenance(
    package: &McdPackage,
    manifest: &Manifest,
) -> Result<Option<ProvenanceMetadata>> {
    let Some(path) = &manifest.provenance else {
        return Ok(None);
    };
    let metadata = ProvenanceMetadata::from_package(package, path)?;
    metadata.validate(manifest, package, path)?;
    Ok(Some(metadata))
}

struct ProvenanceIndexes {
    sources: HashSet<String>,
    actors: HashSet<String>,
    tools: HashSet<String>,
    generated_assets: HashSet<String>,
    tables: HashSet<String>,
    images: HashSet<String>,
    annotations: HashSet<String>,
    external_data: HashSet<String>,
    assets: HashSet<String>,
}

impl ProvenanceIndexes {
    fn new(metadata: &ProvenanceMetadata, manifest: &Manifest, source: &str) -> Result<Self> {
        let sources = collect_ids(
            "provenance.source.id.duplicate",
            metadata.sources.iter().map(|item| item.id.as_str()),
            "source",
            source,
        )?;
        let actors = collect_ids(
            "provenance.actor.id.duplicate",
            metadata.actors.iter().map(|item| item.id.as_str()),
            "actor",
            source,
        )?;
        let tools = collect_ids(
            "provenance.tool.id.duplicate",
            metadata.tools.iter().map(|item| item.id.as_str()),
            "tool",
            source,
        )?;
        let generated_assets = collect_ids(
            "provenance.generated_asset.id.duplicate",
            metadata
                .generated_assets
                .iter()
                .map(|item| item.id.as_str()),
            "generated asset",
            source,
        )?;
        collect_ids(
            "provenance.activity.id.duplicate",
            metadata.activities.iter().map(|item| item.id.as_str()),
            "activity",
            source,
        )?;

        Ok(Self {
            sources,
            actors,
            tools,
            generated_assets,
            tables: manifest.tables.iter().map(|item| item.id.clone()).collect(),
            images: manifest.images.iter().map(|item| item.id.clone()).collect(),
            annotations: manifest
                .annotations
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            external_data: manifest
                .external_data
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            assets: manifest
                .assets
                .iter()
                .filter_map(|item| item.id.clone())
                .collect(),
        })
    }
}

fn collect_ids<'a>(
    code: &'static str,
    ids: impl Iterator<Item = &'a str>,
    label: &str,
    source: &str,
) -> Result<HashSet<String>> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.to_owned()) {
            return Err(provenance_error(
                code,
                format!("Duplicate provenance {label} id '{id}'."),
                source,
            ));
        }
    }
    Ok(seen)
}

fn validate_id(code: &'static str, id: &str, source: &str) -> Result<()> {
    if !is_valid_id(id) {
        return Err(provenance_error(
            code,
            format!("Invalid provenance id '{id}'."),
            source,
        ));
    }
    Ok(())
}

fn validate_package_path(code: &'static str, path: &str, source: &str) -> Result<()> {
    validate_internal_path(path).map(|_| ()).map_err(|_| {
        provenance_error(
            code,
            format!("Invalid provenance package path '{path}'."),
            source,
        )
    })
}

fn validate_uri(code: &'static str, uri: &str, source: &str) -> Result<()> {
    if !is_supported_external_uri(uri) {
        return Err(provenance_error(
            code,
            format!("Invalid provenance URI '{uri}'."),
            source,
        ));
    }
    Ok(())
}

fn validate_optional_media_type(
    value: &Option<String>,
    code: &'static str,
    source: &str,
) -> Result<()> {
    if let Some(value) = value
        && !is_media_type(value)
    {
        return Err(provenance_error(
            code,
            format!("Invalid provenance media type '{value}'."),
            source,
        ));
    }
    Ok(())
}

fn validate_optional_hash(value: &Option<String>, code: &'static str, source: &str) -> Result<()> {
    if let Some(value) = value
        && !is_sha256(value)
    {
        return Err(provenance_error(
            code,
            "Provenance hash must use sha256:<64 lowercase hex characters>.",
            source,
        ));
    }
    Ok(())
}

fn validate_optional_text(value: &Option<String>, code: &'static str, source: &str) -> Result<()> {
    if value
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(provenance_error(
            code,
            "Provenance text fields cannot be empty.",
            source,
        ));
    }
    Ok(())
}

fn validate_optional_timestamp(
    value: &Option<String>,
    code: &'static str,
    source: &str,
) -> Result<Option<OffsetDateTime>> {
    let Some(value) = value else {
        return Ok(None);
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| {
            provenance_error(
                code,
                format!("Provenance timestamp '{value}' must be RFC 3339 date-time."),
                source,
            )
        })
}

fn validate_refs(
    code: &'static str,
    refs: &[String],
    allowed: &HashSet<String>,
    label: &str,
    source: &str,
) -> Result<()> {
    let mut seen = HashSet::new();
    for reference in refs {
        if reference.trim().is_empty() {
            return Err(provenance_error(
                code,
                format!("Provenance {label} reference cannot be empty."),
                source,
            ));
        }
        if !seen.insert(reference) {
            return Err(provenance_error(
                code,
                format!("Duplicate provenance {label} reference '{reference}'."),
                source,
            ));
        }
        if !allowed.contains(reference) {
            return Err(provenance_error(
                code,
                format!("Provenance {label} reference '{reference}' is unresolved."),
                source,
            ));
        }
    }
    Ok(())
}

fn validate_generic_refs(
    code: &'static str,
    refs: &[String],
    indexes: &ProvenanceIndexes,
    source: &str,
) -> Result<()> {
    let mut seen = HashSet::new();
    for reference in refs {
        if reference.trim().is_empty() {
            return Err(provenance_error(
                code,
                "Provenance activity references cannot be empty.",
                source,
            ));
        }
        if !seen.insert(reference) {
            return Err(provenance_error(
                code,
                format!("Duplicate provenance activity reference '{reference}'."),
                source,
            ));
        }
        if let Some((kind, id)) = reference.split_once(':') {
            validate_typed_ref(code, kind, id, indexes, source)?;
        }
    }
    Ok(())
}

fn validate_typed_ref(
    code: &'static str,
    kind: &str,
    id: &str,
    indexes: &ProvenanceIndexes,
    source: &str,
) -> Result<()> {
    if id.trim().is_empty() {
        return Err(provenance_error(
            code,
            "Typed provenance references must include an id.",
            source,
        ));
    }

    let resolved = match kind {
        "source" => indexes.sources.contains(id),
        "actor" => indexes.actors.contains(id),
        "tool" => indexes.tools.contains(id),
        "generatedAsset" | "generated_asset" => indexes.generated_assets.contains(id),
        "table" => indexes.tables.contains(id),
        "image" => indexes.images.contains(id),
        "annotation" => indexes.annotations.contains(id),
        "externalData" | "external_data" => indexes.external_data.contains(id),
        "asset" => indexes.assets.contains(id),
        "path" => validate_internal_path(id).is_ok(),
        _ => true,
    };

    if !resolved {
        return Err(provenance_error(
            code,
            format!("Typed provenance reference '{kind}:{id}' is unresolved."),
            source,
        ));
    }
    Ok(())
}

fn provenance_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn loads_and_validates_provenance_metadata() {
        let package = package_with_provenance(valid_provenance());
        let manifest = package.manifest().expect("manifest");
        let provenance = load_manifest_provenance(&package, &manifest)
            .expect("provenance loads")
            .expect("provenance is present");

        assert_eq!(provenance.sources[0].id, "source-pdf");
        assert_eq!(provenance.generated_assets[0].path, "content/main.md");
    }

    #[test]
    fn rejects_unresolved_provenance_source_reference() {
        let package = package_with_provenance(
            r#"{
                "generatedAssets":[{
                    "id":"main-md",
                    "path":"content/main.md",
                    "sourceRefs":["missing-source"]
                }]
            }"#,
        );
        let manifest = package.manifest().expect("manifest");
        let err = load_manifest_provenance(&package, &manifest)
            .expect_err("unresolved source reference should fail");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("provenance.generated_asset.source_ref.unresolved")
        );
    }

    #[test]
    fn rejects_invalid_provenance_timestamp() {
        let package = package_with_provenance(
            r#"{
                "sources":[{
                    "id":"source-pdf",
                    "path":"assets/source.pdf",
                    "createdAt":"2026-05-26"
                }]
            }"#,
        );
        let manifest = package.manifest().expect("manifest");
        let err = load_manifest_provenance(&package, &manifest)
            .expect_err("date without time zone should fail");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("provenance.source.created_at.invalid")
        );
    }

    fn valid_provenance() -> &'static str {
        r#"{
            "sources":[{
                "id":"source-pdf",
                "path":"assets/source.pdf",
                "mediaType":"application/pdf",
                "hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "createdAt":"2026-05-26T10:00:00Z"
            }],
            "actors":[{"id":"agent-1","kind":"agent","name":"Extraction agent"}],
            "tools":[{"id":"extractor","name":"mcd-pdf-extract","version":"0.1.0"}],
            "generatedAssets":[{
                "id":"main-md",
                "path":"content/main.md",
                "mediaType":"text/markdown",
                "createdAt":"2026-05-26T10:01:00Z",
                "sourceRefs":["source-pdf"],
                "toolRefs":["extractor"],
                "actorRefs":["agent-1"]
            }],
            "activities":[{
                "id":"extract-1",
                "kind":"extracted",
                "startedAt":"2026-05-26T10:00:00Z",
                "endedAt":"2026-05-26T10:01:00Z",
                "sourceRefs":["source-pdf"],
                "toolRefs":["extractor"],
                "actorRefs":["agent-1"],
                "inputRefs":["source:source-pdf"],
                "outputRefs":["generatedAsset:main-md", "path:content/main.md"]
            }]
        }"#
    }

    fn package_with_provenance(provenance: &str) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{
                    "format":"MCD",
                    "version":"0.1",
                    "profile":"MCD-Core",
                    "entrypoint":"content/main.md",
                    "assets":[{"id":"source-pdf","path":"assets/source.pdf"}],
                    "provenance":"provenance/provenance.json"
                }"#,
            ),
            ("content/main.md", "# Source\n"),
            ("assets/source.pdf", "%PDF-1.7\n"),
            ("provenance/provenance.json", provenance),
        ]))
        .expect("package opens")
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
