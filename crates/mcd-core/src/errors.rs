//! Error and diagnostic types shared across the core crate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type used by the MCD core crate.
pub type Result<T> = std::result::Result<T, McdError>;

/// A stable validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional source path and location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Related source paths and locations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
}

impl Diagnostic {
    /// Construct an error diagnostic.
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            code: code.into(),
            message: message.into(),
            source: None,
            related: Vec::new(),
        }
    }

    /// Attach a source reference.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Attach a related source reference.
    #[must_use]
    pub fn with_related(mut self, related: impl Into<String>) -> Self {
        self.related.push(related.into());
        self
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    /// Error severity.
    Error,
    /// Warning severity.
    Warning,
    /// Informational severity.
    Info,
}

/// Fatal errors returned by parser operations.
#[derive(Debug, Error)]
pub enum McdError {
    /// I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// ZIP archive failure.
    #[error("package archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// JSON failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// UTF-8 failure.
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// Structured validation failure.
    #[error("{diagnostic_code}: {diagnostic_message}")]
    Diagnostic {
        /// Stable diagnostic code.
        diagnostic_code: String,
        /// Human-readable diagnostic message.
        diagnostic_message: String,
        /// Full diagnostic.
        diagnostic: Box<Diagnostic>,
    },
}

impl McdError {
    /// Convert a diagnostic into a fatal error.
    #[must_use]
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic {
            diagnostic_code: diagnostic.code.clone(),
            diagnostic_message: diagnostic.message.clone(),
            diagnostic: Box::new(diagnostic),
        }
    }

    /// Borrow the structured diagnostic when available.
    #[must_use]
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        match self {
            Self::Diagnostic { diagnostic, .. } => Some(diagnostic.as_ref()),
            Self::Io(_) | Self::Zip(_) | Self::Json(_) | Self::Utf8(_) => None,
        }
    }
}
