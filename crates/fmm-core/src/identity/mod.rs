use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Component, Path};

/// Parser cache version embedded in per-file fingerprints.
///
/// Bump this when parser output semantics change without a source file content
/// change.
pub const PARSER_CACHE_VERSION: u32 = 1;

/// Result type for identity primitive operations.
pub type Result<T> = std::result::Result<T, IdentityError>;

/// Errors raised while adapting filesystem paths into fmm's internal identity
/// primitives.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// A path could not be canonicalized before identity normalization.
    #[error("failed to canonicalize {kind} path {path}: {source}")]
    Canonicalize {
        kind: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The source path is not contained by the indexing root.
    #[error("path {path} is not relative to root {root}")]
    OutsideRoot { root: String, path: String },

    /// fmm public path contracts require UTF-8 relative paths.
    #[error("relative path cannot be represented as UTF-8")]
    NonUtf8,

    /// Canonical relative paths must not contain absolute or parent segments.
    #[error("relative path contains a non-normal component")]
    NonNormalComponent,
}

/// Dense internal file identity.
///
/// `FileId` is assigned from sorted canonical relative paths by downstream graph
/// work. It is intentionally opaque at CLI and MCP boundaries, where fmm keeps
/// path based contracts.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileId(pub u32);

const _: () = assert!(std::mem::size_of::<FileId>() == 4);

/// Slash separated path relative to an indexing root.
///
/// This is the path form used to bridge public path based APIs and internal
/// identity tables. It must be UTF-8 because fmm stores and emits paths as text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelativePath(String);

impl RelativePath {
    /// Borrow the normalized relative path text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Durable cache identity for a parsed source file.
///
/// Downstream invalidation first compares source metadata and parser cache
/// version, then falls back to `content_hash` when metadata changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Source modification time in fmm's persisted timestamp format.
    pub source_mtime: String,
    /// Source file byte size from filesystem metadata.
    pub source_size: u64,
    /// Stable content hash used when metadata alone cannot prove freshness.
    pub content_hash: String,
    /// Parser cache schema version that invalidates stale parse results.
    pub parser_cache_version: u32,
}

/// Dependency edge classification used by graph storage and cycle diagnostics.
///
/// `Runtime` edges affect runtime dependency traversal. `TypeOnly` edges are
/// preserved for structural queries but may be skipped by runtime cycle reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Runtime dependency edge.
    Runtime,
    /// Type-only dependency edge, for example TypeScript `import type`.
    TypeOnly,
}

/// Convert an absolute path into fmm's canonical slash separated relative form.
///
/// Both paths are canonicalized before stripping so symlinks and redundant path
/// components do not create duplicate identities.
pub fn normalize_relative(root: impl AsRef<Path>, abs: impl AsRef<Path>) -> Result<RelativePath> {
    let root = root.as_ref();
    let abs = abs.as_ref();
    let canonical_root = canonicalize("root", root)?;
    let canonical_abs = canonicalize("source", abs)?;
    let relative =
        canonical_abs
            .strip_prefix(&canonical_root)
            .map_err(|_| IdentityError::OutsideRoot {
                root: canonical_root.display().to_string(),
                path: canonical_abs.display().to_string(),
            })?;

    let parts = relative
        .components()
        .map(normal_component_text)
        .collect::<Result<Vec<_>>>()?;

    Ok(RelativePath(parts.join("/")))
}

fn canonicalize(kind: &'static str, path: &Path) -> Result<std::path::PathBuf> {
    std::fs::canonicalize(path).map_err(|source| IdentityError::Canonicalize {
        kind,
        path: path.display().to_string(),
        source,
    })
}

fn normal_component_text(component: Component<'_>) -> Result<String> {
    match component {
        Component::Normal(value) => value
            .to_str()
            .map(str::to_owned)
            .ok_or(IdentityError::NonUtf8),
        Component::CurDir => Ok(String::new()),
        Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
            Err(IdentityError::NonNormalComponent)
        }
    }
}
