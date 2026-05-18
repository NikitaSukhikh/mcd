//! Annotation metadata parsing and validation.

use std::collections::HashSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    Manifest, McdPackage,
    document::{DocumentBlock, McdDocument, SourceSpan},
    errors::{Diagnostic, McdError, Result},
    manifest::AnnotationManifestEntry,
    package::validate_internal_path,
};

/// Parsed annotation metadata object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationMetadata {
    /// Stable annotation id.
    pub id: String,
    /// Annotation target.
    pub target: AnnotationTarget,
    /// Annotation kind.
    pub kind: AnnotationKind,
    /// Review lifecycle status.
    pub status: AnnotationStatus,
    /// Human/agent-readable annotation body.
    pub body: String,
    /// Optional author identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional creation timestamp string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Optional version-control-friendly labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Optional proposed textual change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_change: Option<ProposedChange>,
}

impl AnnotationMetadata {
    /// Parse annotation metadata from a package entry.
    pub fn from_package(package: &McdPackage, path: &str) -> Result<Self> {
        let bytes = package.read(path).map_err(|_| {
            McdError::from_diagnostic(
                Diagnostic::error(
                    "annotation.metadata.missing",
                    format!("Declared annotation metadata file '{path}' is missing."),
                )
                .with_source(path.to_owned()),
            )
        })?;
        serde_json::from_slice::<Self>(bytes).map_err(McdError::from)
    }

    /// Validate annotation metadata, target references, and proposed changes.
    pub fn validate(
        &self,
        expected_id: &str,
        manifest: &Manifest,
        package: &McdPackage,
        document: &McdDocument,
        source: &str,
    ) -> Result<()> {
        if self.id != expected_id {
            return Err(annotation_error(
                "annotation.id.mismatch",
                format!(
                    "Annotation metadata id '{}' does not match manifest annotation id '{}'.",
                    self.id, expected_id
                ),
                source,
            ));
        }
        if self.id.trim().is_empty() {
            return Err(annotation_error(
                "annotation.id.empty",
                "Annotation metadata id cannot be empty.",
                source,
            ));
        }
        if self.body.trim().is_empty() {
            return Err(annotation_error(
                "annotation.body.empty",
                "Annotation body cannot be empty.",
                source,
            ));
        }
        for label in &self.labels {
            if label.trim().is_empty() {
                return Err(annotation_error(
                    "annotation.label.empty",
                    "Annotation labels cannot be empty.",
                    source,
                ));
            }
        }

        validate_target(&self.target, manifest, package, document, source)?;
        if let Some(change) = &self.proposed_change {
            change.validate(source)?;
        }

        Ok(())
    }
}

/// Supported annotation targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnnotationTarget {
    /// Whole-document annotation.
    Document,
    /// Generated canonical block id target.
    Block {
        /// Canonical block id.
        id: String,
    },
    /// Stable placement ref target from a table or image directive.
    Placement {
        /// Placement ref.
        #[serde(rename = "ref")]
        ref_id: String,
    },
    /// Manifest-declared table id target.
    Table {
        /// Table id.
        id: String,
    },
    /// Manifest-declared image id target.
    Image {
        /// Image id.
        id: String,
    },
    /// Package path target, optionally with a source span.
    Path {
        /// Package path.
        path: String,
        /// Optional span within the path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
}

/// Annotation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    /// General comment.
    Comment,
    /// Review flag.
    Flag,
    /// Proposed edit.
    ProposedChange,
    /// Question for a human or agent.
    Question,
    /// Follow-up task.
    Todo,
}

/// Annotation review status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationStatus {
    /// Open annotation.
    Open,
    /// Accepted annotation or change.
    Accepted,
    /// Rejected annotation or change.
    Rejected,
    /// Resolved annotation.
    Resolved,
}

/// Textual proposed change metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedChange {
    /// Package path to modify.
    pub path: String,
    /// Optional replacement span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace: Option<SourceSpan>,
    /// Proposed replacement or insertion text.
    pub text: String,
}

impl ProposedChange {
    fn validate(&self, source: &str) -> Result<()> {
        validate_internal_path(&self.path).map_err(|_| {
            annotation_error(
                "annotation.proposed_change.path.invalid",
                format!("Invalid proposed change path '{}'.", self.path),
                source,
            )
        })?;
        if self.text.trim().is_empty() {
            return Err(annotation_error(
                "annotation.proposed_change.text.empty",
                "Proposed change text cannot be empty.",
                source,
            ));
        }
        Ok(())
    }
}

/// Load and validate all manifest-declared annotations.
pub fn load_manifest_annotations(
    package: &McdPackage,
    manifest: &Manifest,
    document: &McdDocument,
) -> Result<IndexMap<String, AnnotationMetadata>> {
    let mut annotations = IndexMap::new();
    for entry in &manifest.annotations {
        let annotation = load_manifest_annotation(package, manifest, document, entry)?;
        annotations.insert(entry.id.clone(), annotation);
    }
    Ok(annotations)
}

/// Validate all Markdown annotation markers against declared annotation metadata.
pub fn validate_annotation_markers(
    document: &McdDocument,
    annotations: &IndexMap<String, AnnotationMetadata>,
) -> Result<()> {
    for block in &document.blocks {
        for annotation_ref in block.annotation_refs() {
            if !annotations.contains_key(&annotation_ref.id) {
                return Err(annotation_marker_error(
                    "annotation.marker.unresolved",
                    format!(
                        "Markdown annotation marker references undeclared annotation '{}'.",
                        annotation_ref.id
                    ),
                    document,
                    block_source(block),
                ));
            }
        }
    }
    Ok(())
}

fn load_manifest_annotation(
    package: &McdPackage,
    manifest: &Manifest,
    document: &McdDocument,
    entry: &AnnotationManifestEntry,
) -> Result<AnnotationMetadata> {
    let annotation = AnnotationMetadata::from_package(package, &entry.metadata)?;
    annotation.validate(&entry.id, manifest, package, document, &entry.metadata)?;
    Ok(annotation)
}

fn validate_target(
    target: &AnnotationTarget,
    manifest: &Manifest,
    package: &McdPackage,
    document: &McdDocument,
    source: &str,
) -> Result<()> {
    match target {
        AnnotationTarget::Document => Ok(()),
        AnnotationTarget::Block { id } => {
            if document.blocks.iter().any(|block| block.id() == id) {
                Ok(())
            } else {
                Err(annotation_error(
                    "annotation.target.block.unresolved",
                    format!("Annotation target references unknown block id '{id}'."),
                    source,
                ))
            }
        }
        AnnotationTarget::Placement { ref_id } => {
            if placement_refs(document).contains(ref_id) {
                Ok(())
            } else {
                Err(annotation_error(
                    "annotation.target.placement.unresolved",
                    format!("Annotation target references unknown placement ref '{ref_id}'."),
                    source,
                ))
            }
        }
        AnnotationTarget::Table { id } => {
            if manifest.tables.iter().any(|table| table.id == *id) {
                Ok(())
            } else {
                Err(annotation_error(
                    "annotation.target.table.unresolved",
                    format!("Annotation target references unknown table id '{id}'."),
                    source,
                ))
            }
        }
        AnnotationTarget::Image { id } => {
            if manifest.images.iter().any(|image| image.id == *id) {
                Ok(())
            } else {
                Err(annotation_error(
                    "annotation.target.image.unresolved",
                    format!("Annotation target references unknown image id '{id}'."),
                    source,
                ))
            }
        }
        AnnotationTarget::Path { path, .. } => {
            validate_internal_path(path).map_err(|_| {
                annotation_error(
                    "annotation.target.path.invalid",
                    format!("Invalid annotation target path '{path}'."),
                    source,
                )
            })?;
            if package.contains(path) {
                Ok(())
            } else {
                Err(annotation_error(
                    "annotation.target.path.missing",
                    format!("Annotation target path '{path}' is missing from the package."),
                    source,
                ))
            }
        }
    }
}

fn placement_refs(document: &McdDocument) -> HashSet<String> {
    document
        .blocks
        .iter()
        .filter_map(|block| match block {
            DocumentBlock::TableRef { placement, .. } => placement.ref_id.clone(),
            DocumentBlock::ImageRef { placement, .. } => placement.ref_id.clone(),
            _ => None,
        })
        .collect()
}

fn block_source(block: &DocumentBlock) -> Option<SourceSpan> {
    match block {
        DocumentBlock::Heading { source, .. }
        | DocumentBlock::Paragraph { source, .. }
        | DocumentBlock::List { source, .. }
        | DocumentBlock::CodeBlock { source, .. }
        | DocumentBlock::Quote { source, .. }
        | DocumentBlock::MathBlock { source, .. }
        | DocumentBlock::TableRef { source, .. }
        | DocumentBlock::ImageRef { source, .. } => *source,
    }
}

fn annotation_marker_error(
    code: impl Into<String>,
    message: impl Into<String>,
    document: &McdDocument,
    source: Option<SourceSpan>,
) -> McdError {
    let source = source
        .map(|span| format!("{}:{span}", document.source_path))
        .unwrap_or_else(|| document.source_path.clone());
    annotation_error(code, message, &source)
}

fn annotation_error(code: impl Into<String>, message: impl Into<String>, source: &str) -> McdError {
    McdError::from_diagnostic(Diagnostic::error(code, message).with_source(source.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    #[test]
    fn validates_annotation_targeting_placement_ref() {
        let package = package_with_annotation(
            r#"{
                "id":"review-revenue-chart",
                "target":{"type":"placement","ref":"revenue-chart"},
                "kind":"flag",
                "status":"open",
                "body":"Check whether Q1 revenue needs a footnote.",
                "labels":["finance","review"]
            }"#,
            ":::table\nref: revenue-chart\ntable: revenue\n:::\n",
        );
        let manifest = package.manifest().expect("manifest");
        let document = McdDocument::from_package(&package, &manifest).expect("document");
        let annotations = load_manifest_annotations(&package, &manifest, &document)
            .expect("annotations validate");

        assert_eq!(annotations.len(), 1);
        assert_eq!(
            annotations["review-revenue-chart"].kind,
            AnnotationKind::Flag
        );
    }

    #[test]
    fn rejects_unresolved_annotation_target() {
        let package = package_with_annotation(
            r#"{
                "id":"review-revenue-chart",
                "target":{"type":"placement","ref":"missing"},
                "kind":"flag",
                "status":"open",
                "body":"Check this chart."
            }"#,
            ":::table\nref: revenue-chart\ntable: revenue\n:::\n",
        );
        let manifest = package.manifest().expect("manifest");
        let document = McdDocument::from_package(&package, &manifest).expect("document");
        let err = load_manifest_annotations(&package, &manifest, &document)
            .expect_err("annotation target should fail");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("annotation.target.placement.unresolved")
        );
    }

    #[test]
    fn rejects_unresolved_markdown_annotation_marker() {
        let package = McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            (
                "manifest.json",
                r#"{
                    "format":"MCD",
                    "version":"0.1",
                    "profile":"MCD-Core",
                    "entrypoint":"content/main.md"
                }"#,
            ),
            (
                "content/main.md",
                "Revenue[[annotation:missing-note]] increased.\n",
            ),
        ]))
        .expect("package opens");
        let manifest = package.manifest().expect("manifest");
        let document = McdDocument::from_package(&package, &manifest).expect("document");
        let annotations = load_manifest_annotations(&package, &manifest, &document)
            .expect("empty annotation set loads");
        let err =
            validate_annotation_markers(&document, &annotations).expect_err("marker should fail");

        assert_eq!(
            err.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
            Some("annotation.marker.unresolved")
        );
    }

    fn package_with_annotation(annotation_json: &str, markdown: &str) -> McdPackage {
        McdPackage::from_bytes(&zip_bytes(&[
            ("mimetype", crate::package::MCD_MIMETYPE),
            ("manifest.json", manifest()),
            ("content/main.md", markdown),
            ("tables/revenue.csv", "quarter,revenue_gbp\nQ1,125000.00\n"),
            (
                "tables/revenue.schema.json",
                r#"{"id":"revenue","columns":[{"name":"quarter","type":"string"},{"name":"revenue_gbp","type":"decimal"}]}"#,
            ),
            (
                "annotations/review-revenue-chart.annotation.json",
                annotation_json,
            ),
        ]))
        .expect("package opens")
    }

    fn manifest() -> &'static str {
        r#"{
            "format":"MCD",
            "version":"0.1",
            "profile":"MCD-Core",
            "entrypoint":"content/main.md",
            "tables":[{"id":"revenue","data":"tables/revenue.csv","schema":"tables/revenue.schema.json"}],
            "annotations":[{"id":"review-revenue-chart","metadata":"annotations/review-revenue-chart.annotation.json"}]
        }"#
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
