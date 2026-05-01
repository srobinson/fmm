use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Component, Path};

/// Parser cache version embedded in per-file fingerprints.
///
/// Bump this when parser output semantics change without a source file content
/// change.
pub const PARSER_CACHE_VERSION: u32 = 2;

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

    /// A persisted identity map cannot contain the same id twice.
    #[error("duplicate file id {0}")]
    DuplicateFileId(u32),

    /// A persisted identity map cannot contain the same path twice.
    #[error("duplicate relative path {0}")]
    DuplicateRelativePath(String),
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
    /// Build a relative path from fmm's slash separated storage key form.
    pub fn from_slash_path(path: impl AsRef<str>) -> Result<Self> {
        let path = path.as_ref();
        if path.is_empty() || path.contains('\\') || Path::new(path).is_absolute() {
            return Err(IdentityError::NonNormalComponent);
        }

        let mut parts = Vec::new();
        for component in Path::new(path).components() {
            let Component::Normal(value) = component else {
                return Err(IdentityError::NonNormalComponent);
            };
            parts.push(
                value
                    .to_str()
                    .map(str::to_owned)
                    .ok_or(IdentityError::NonUtf8)?,
            );
        }

        if parts.is_empty() {
            return Err(IdentityError::NonNormalComponent);
        }

        Ok(Self(parts.join("/")))
    }

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

/// Bidirectional mapping between fmm path keys and dense internal file ids.
///
/// Full indexing builds a compact map from sorted paths. Incremental watch
/// updates append new ids and leave removed ids vacant so survivor ids remain
/// stable until the next full indexing boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileIdentityMap {
    path_to_id: HashMap<RelativePath, FileId>,
    id_to_path: Vec<Option<RelativePath>>,
}

impl FileIdentityMap {
    /// Build a deterministic dense map from absolute source paths.
    pub fn from_absolute_paths<I, P>(root: impl AsRef<Path>, paths: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let root = root.as_ref();
        let paths = paths
            .into_iter()
            .map(|path| normalize_relative(root, path))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self::from_relative_values(paths))
    }

    /// Build a deterministic dense map from slash separated relative paths.
    pub fn from_relative_paths<I, P>(paths: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<str>,
    {
        let paths = paths
            .into_iter()
            .map(RelativePath::from_slash_path)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self::from_relative_values(paths))
    }

    /// Build a map from persisted file id and relative path pairs.
    pub fn from_file_id_paths<I, P>(entries: I) -> Result<Self>
    where
        I: IntoIterator<Item = (FileId, P)>,
        P: AsRef<str>,
    {
        let mut path_to_id = HashMap::new();
        let mut id_to_path = Vec::new();

        for (id, path) in entries {
            let path = RelativePath::from_slash_path(path)?;
            let index = id.0 as usize;
            if id_to_path.len() <= index {
                id_to_path.resize_with(index + 1, || None);
            }
            if id_to_path[index].is_some() {
                return Err(IdentityError::DuplicateFileId(id.0));
            }
            if path_to_id.contains_key(&path) {
                return Err(IdentityError::DuplicateRelativePath(
                    path.as_str().to_string(),
                ));
            }

            id_to_path[index] = Some(path.clone());
            path_to_id.insert(path, id);
        }

        Ok(Self {
            path_to_id,
            id_to_path,
        })
    }

    /// Return the id currently assigned to a path key.
    pub fn id_for_path(&self, path: &str) -> Option<FileId> {
        let path = RelativePath::from_slash_path(path).ok()?;
        self.path_to_id.get(&path).copied()
    }

    /// Return the path key currently assigned to an id.
    pub fn path_for_id(&self, id: FileId) -> Option<&RelativePath> {
        self.id_to_path.get(id.0 as usize).and_then(Option::as_ref)
    }

    /// Preserve an existing path id or append a new id for a watch-created path.
    pub fn ensure_relative_path(&mut self, path: impl AsRef<str>) -> Result<FileId> {
        let path = RelativePath::from_slash_path(path)?;
        if let Some(id) = self.path_to_id.get(&path) {
            return Ok(*id);
        }

        let id = FileId(self.id_to_path.len() as u32);
        self.id_to_path.push(Some(path.clone()));
        self.path_to_id.insert(path, id);
        Ok(id)
    }

    /// Remove a path while preserving all other assigned ids.
    pub fn remove_relative_path(&mut self, path: impl AsRef<str>) -> Result<Option<FileId>> {
        let path = RelativePath::from_slash_path(path)?;
        let Some(id) = self.path_to_id.remove(&path) else {
            return Ok(None);
        };
        if let Some(slot) = self.id_to_path.get_mut(id.0 as usize) {
            *slot = None;
        }
        Ok(Some(id))
    }

    fn from_relative_values(mut paths: Vec<RelativePath>) -> Self {
        paths.sort();
        paths.dedup();

        let mut path_to_id = HashMap::with_capacity(paths.len());
        let mut id_to_path = Vec::with_capacity(paths.len());
        for (index, path) in paths.into_iter().enumerate() {
            let id = FileId(index as u32);
            path_to_id.insert(path.clone(), id);
            id_to_path.push(Some(path));
        }

        Self {
            path_to_id,
            id_to_path,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{FileId, FileIdentityMap, IdentityError, RelativePath};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn from_slash_path_rejects_non_normal_inputs() {
        let cases = [
            "",
            "/abs/path",
            "src\\a.ts",
            ".",
            "..",
            "./src/a.ts",
            "../src/a.ts",
            "src/../a.ts",
        ];

        for path in cases {
            let result = RelativePath::from_slash_path(path);
            assert!(
                matches!(result, Err(IdentityError::NonNormalComponent)),
                "expected NonNormalComponent for {path:?}, got {result:?}"
            );
        }
    }

    #[test]
    fn file_ids_are_assigned_from_sorted_normalized_absolute_paths() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let src = root.join("src");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("z.ts"), "").unwrap();
        fs::write(src.join("a.ts"), "").unwrap();
        fs::write(src.join("nested/b.ts"), "").unwrap();

        let identities = FileIdentityMap::from_absolute_paths(
            root,
            [
                src.join("z.ts"),
                src.join("nested/../nested/b.ts"),
                src.join("a.ts"),
            ],
        )
        .unwrap();

        assert_eq!(identities.id_for_path("src/a.ts"), Some(FileId(0)));
        assert_eq!(identities.id_for_path("src/nested/b.ts"), Some(FileId(1)));
        assert_eq!(identities.id_for_path("src/z.ts"), Some(FileId(2)));
        assert_eq!(
            identities.path_for_id(FileId(1)).map(|path| path.as_str()),
            Some("src/nested/b.ts")
        );
    }

    #[test]
    fn incremental_identity_updates_preserve_survivor_ids() {
        let mut identities =
            FileIdentityMap::from_relative_paths(["src/a.ts", "src/b.ts", "src/c.ts"]).unwrap();

        let a_id = identities.id_for_path("src/a.ts").unwrap();
        let b_id = identities.id_for_path("src/b.ts").unwrap();
        let c_id = identities.id_for_path("src/c.ts").unwrap();

        assert_eq!(identities.ensure_relative_path("src/b.ts").unwrap(), b_id);
        assert_eq!(
            identities.ensure_relative_path("src/d.ts").unwrap(),
            FileId(3)
        );
        assert_eq!(
            identities.remove_relative_path("src/b.ts").unwrap(),
            Some(b_id)
        );
        assert_eq!(
            identities.ensure_relative_path("src/e.ts").unwrap(),
            FileId(4)
        );

        assert_eq!(identities.id_for_path("src/a.ts"), Some(a_id));
        assert_eq!(identities.id_for_path("src/c.ts"), Some(c_id));
        assert_eq!(identities.path_for_id(b_id), None);
        assert_eq!(
            identities.path_for_id(a_id).map(|path| path.as_str()),
            Some("src/a.ts")
        );
    }

    #[test]
    fn from_file_id_paths_preserves_sparse_ids() {
        let identities =
            FileIdentityMap::from_file_id_paths([(FileId(0), "src/a.ts"), (FileId(3), "src/d.ts")])
                .unwrap();

        assert_eq!(identities.id_for_path("src/a.ts"), Some(FileId(0)));
        assert_eq!(identities.id_for_path("src/d.ts"), Some(FileId(3)));
        assert_eq!(identities.path_for_id(FileId(1)), None);
        assert_eq!(identities.path_for_id(FileId(2)), None);
        assert_eq!(
            identities.path_for_id(FileId(3)).map(RelativePath::as_str),
            Some("src/d.ts")
        );
    }

    #[test]
    fn from_file_id_paths_accepts_empty_input() {
        let identities =
            FileIdentityMap::from_file_id_paths(std::iter::empty::<(FileId, &str)>()).unwrap();

        assert_eq!(identities.id_for_path("src/a.ts"), None);
        assert_eq!(identities.path_for_id(FileId(0)), None);
    }

    #[test]
    fn from_file_id_paths_rejects_duplicate_file_id() {
        let result =
            FileIdentityMap::from_file_id_paths([(FileId(0), "src/a.ts"), (FileId(0), "src/b.ts")]);

        assert!(
            matches!(result, Err(IdentityError::DuplicateFileId(0))),
            "expected DuplicateFileId(0), got {result:?}"
        );
    }

    #[test]
    fn from_file_id_paths_rejects_duplicate_path() {
        let result =
            FileIdentityMap::from_file_id_paths([(FileId(0), "src/a.ts"), (FileId(1), "src/a.ts")]);

        match result {
            Err(IdentityError::DuplicateRelativePath(path)) => assert_eq!(path, "src/a.ts"),
            other => panic!("expected DuplicateRelativePath, got {other:?}"),
        }
    }

    #[test]
    fn from_file_id_paths_rejects_non_normal_paths() {
        let cases = ["", "/abs/path", "..", "./src/a.ts", "src/../a.ts"];
        for path in cases {
            let result = FileIdentityMap::from_file_id_paths([(FileId(0), path)]);
            assert!(
                matches!(result, Err(IdentityError::NonNormalComponent)),
                "expected NonNormalComponent for {path:?}, got {result:?}"
            );
        }
    }
}
