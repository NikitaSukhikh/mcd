//! Canonical document stream types.

use serde::{Deserialize, Serialize};

use crate::{Manifest, McdPackage, markdown};

/// Parsed MCD document with a canonical block stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McdDocument {
    /// Source Markdown path inside the package.
    pub source_path: String,
    /// Blocks in source order.
    pub blocks: Vec<DocumentBlock>,
}

impl McdDocument {
    /// Parse the manifest entrypoint Markdown from a package.
    pub fn from_package(package: &McdPackage, manifest: &Manifest) -> crate::Result<Self> {
        let markdown = package.read_to_string(&manifest.entrypoint)?;
        markdown::parse_markdown(&manifest.entrypoint, &markdown)
    }
}

/// A canonical document block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentBlock {
    /// Markdown heading.
    Heading {
        /// Stable generated block id.
        id: String,
        /// Heading level.
        level: u8,
        /// Plain heading text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// Markdown paragraph.
    Paragraph {
        /// Stable generated block id.
        id: String,
        /// Plain paragraph text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// Markdown list.
    List {
        /// Stable generated block id.
        id: String,
        /// Plain list text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// Markdown code block.
    CodeBlock {
        /// Stable generated block id.
        id: String,
        /// Optional language/info string.
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        /// Literal code text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// Markdown block quote.
    Quote {
        /// Stable generated block id.
        id: String,
        /// Plain quote text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// Markdown display math block.
    MathBlock {
        /// Stable generated block id.
        id: String,
        /// Literal math text.
        text: String,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// MCD table placement.
    TableRef {
        /// Stable generated block id.
        id: String,
        /// Parsed table placement.
        placement: crate::directives::TablePlacement,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
    /// MCD image placement.
    ImageRef {
        /// Stable generated block id.
        id: String,
        /// Parsed image placement.
        placement: crate::directives::ImagePlacement,
        /// Source span when available.
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<SourceSpan>,
    },
}

impl DocumentBlock {
    /// Return the stable block id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Heading { id, .. }
            | Self::Paragraph { id, .. }
            | Self::List { id, .. }
            | Self::CodeBlock { id, .. }
            | Self::Quote { id, .. }
            | Self::MathBlock { id, .. }
            | Self::TableRef { id, .. }
            | Self::ImageRef { id, .. } => id,
        }
    }
}

/// 1-based source span in the Markdown entrypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    /// First line.
    pub start_line: usize,
    /// First column.
    pub start_column: usize,
    /// Last line.
    pub end_line: usize,
    /// Last column.
    pub end_column: usize,
}

impl std::fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_column, self.end_line, self.end_column
        )
    }
}
