//! Error types for the fmm-store crate.

use thiserror::Error;

/// Errors returned by store operations.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Underlying database error (wraps rusqlite).
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// No fmm index found at the expected path.
    #[error("no fmm index found at {path}. Run `fmm generate` first.")]
    NoIndex { path: String },

    /// The stored fmm version does not match the running binary.
    #[error(
        "index was built with fmm v{stored} but you are running v{running}. \
         Run `fmm generate --force` to rebuild."
    )]
    VersionMismatch { stored: String, running: String },

    /// Schema migration failed or schema is corrupt.
    #[error("migration error: {0}")]
    Migration(String),

    /// Serialization or other internal error.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
