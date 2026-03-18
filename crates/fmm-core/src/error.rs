//! Domain error types for fmm-core.
//!
//! New code in fmm-core should prefer `FmmError` over `anyhow::Error`.
//! Existing `anyhow::Result` usage will be migrated incrementally.

use std::path::PathBuf;

/// Domain errors for frontmatter-matters core operations.
#[derive(Debug, thiserror::Error)]
pub enum FmmError {
    /// A referenced file was not found in the manifest index.
    #[error("file not found in index: {path}")]
    FileNotFound { path: String },

    /// A referenced export symbol was not found.
    #[error("export not found: {name}")]
    ExportNotFound { name: String },

    /// Configuration loading or validation failed.
    #[error("config error: {message}")]
    Config { message: String },

    /// Source file parsing failed.
    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },

    /// Cross-package or import resolution failed.
    #[error("resolve error: {message}")]
    Resolve { message: String },

    /// Store/persistence operation failed (wraps the store's associated error).
    #[error("store error: {0}")]
    Store(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Convenience type alias for fmm-core domain results.
pub type FmmResult<T> = Result<T, FmmError>;
