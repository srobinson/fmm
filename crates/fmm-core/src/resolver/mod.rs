//! Cross-package import resolver for accurate downstream dependency graphs.
//!
//! Three-layer resolution applied during `build_reverse_deps()`:
//!
//! - Layer 1: tsconfig paths + baseUrl (via oxc-resolver auto-discovery per file)
//! - Layer 2: Workspace package name aliases (injected into oxc-resolver)
//! - Layer 3: Directory prefix heuristic (React/moduleDirectories pattern)

pub mod workspace;

use oxc_resolver::{AliasValue, ResolveOptions, Resolver, TsconfigDiscovery};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod go;
pub mod rust;
pub use go::GoImportResolver;
pub use rust::RustImportResolver;

/// Resolve an import specifier from a source file into an indexed file path.
///
/// Implementors own one language ecosystem's resolution semantics.
pub trait ImportResolver: Send + Sync {
    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf>;
}

/// Three-layer cross-package import resolver.
///
/// Layer 1 (tsconfig paths + baseUrl) and Layer 2 (workspace package aliases)
/// are handled by oxc-resolver. Layer 3 (directory prefix heuristic) is
/// implemented in `resolve_by_directory_prefix`.
///
/// Clone-safe: cloning shares the underlying `Arc<Resolver>` cache across threads.
#[derive(Clone)]
pub struct CrossPackageResolver {
    inner: Arc<Resolver>,
}

impl CrossPackageResolver {
    /// Construct a resolver that injects workspace package aliases (Layer 2)
    /// and enables per-file tsconfig discovery (Layer 1).
    pub fn new(workspace_packages: &HashMap<String, PathBuf>) -> Self {
        // Layer 2: workspace package name → directory alias entries.
        // e.g. "shared" → /repo/packages/shared, "@myorg/lib" → /repo/packages/lib
        let alias: Vec<(String, Vec<AliasValue>)> = workspace_packages
            .iter()
            .filter_map(|(name, dir)| {
                let dir_str = dir.to_str()?.to_string();
                Some((name.clone(), vec![AliasValue::Path(dir_str)]))
            })
            .collect();

        let resolver = Resolver::new(ResolveOptions {
            extensions: vec![
                ".ts".into(),
                ".tsx".into(),
                ".js".into(),
                ".jsx".into(),
                ".mjs".into(),
                ".cjs".into(),
                ".json".into(),
            ],
            condition_names: vec!["node".into(), "import".into(), "require".into()],
            // Layer 1: per-file tsconfig auto-discovery.
            // NOTE: TsconfigDiscovery::Auto only works with resolve_file(), not resolve().
            tsconfig: Some(TsconfigDiscovery::Auto),
            main_fields: vec!["module".into(), "main".into()],
            alias,
            ..ResolveOptions::default()
        });

        Self {
            inner: Arc::new(resolver),
        }
    }

    /// Resolve a cross-package import specifier to an absolute file path.
    ///
    /// `importer` must be an absolute path to the importing file (not its directory).
    /// This triggers per-file tsconfig discovery (Layer 1). Returns `None` for
    /// unresolvable imports (external npm packages, missing config).
    pub fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        // resolve_file() is required for TsconfigDiscovery::Auto to work
        match self.inner.resolve_file(importer, specifier) {
            Ok(resolution) => Some(resolution.full_path().to_path_buf()),
            Err(oxc_resolver::ResolveError::NotFound(_)) => None,
            Err(e) => {
                // Log other errors (filesystem issues, circular tsconfig extends) at debug level
                #[cfg(debug_assertions)]
                eprintln!(
                    "[fmm debug] resolver: {} → {}: {:?}",
                    importer.display(),
                    specifier,
                    e
                );
                let _ = e;
                None
            }
        }
    }
}

impl ImportResolver for CrossPackageResolver {
    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        CrossPackageResolver::resolve(self, importer, specifier)
    }
}

/// Layer 3: directory prefix heuristic for `moduleDirectories`-style repos.
///
/// Handles imports like `shared/ReactFeatureFlags` where no workspace package
/// is named `shared`, but a directory named `shared` exists under a workspace
/// root. This recovers React's `moduleDirectories: ["packages"]` semantics
/// without parsing the Jest config.
///
/// Only returns a resolved path when the file exists in `manifest_paths` —
/// mandatory false-positive guard against external packages.
pub fn resolve_by_directory_prefix(
    specifier: &str,
    workspace_roots: &[PathBuf],
    manifest_paths: &HashSet<String>,
) -> Option<PathBuf> {
    // Need at least "prefix/remainder"
    let (prefix, remainder) = specifier.split_once('/')?;

    // Find a workspace root whose last path component matches the prefix.
    // e.g. /repo/packages/shared → "shared" matches import prefix "shared"
    let matching_root = workspace_roots.iter().find(|root| {
        root.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == prefix)
            .unwrap_or(false)
    })?;

    // Try standard extension candidates in preference order
    let candidates = [
        format!("{}.ts", remainder),
        format!("{}.tsx", remainder),
        format!("{}.js", remainder),
        format!("{}.jsx", remainder),
        format!("{}/index.ts", remainder),
        format!("{}/index.js", remainder),
    ];

    for candidate in &candidates {
        let resolved = matching_root.join(candidate);
        // Filesystem check first (cheap stat), then manifest lookup
        if resolved.exists()
            && let Some(resolved_str) = resolved.to_str()
            && manifest_paths.contains(resolved_str)
        {
            return Some(resolved);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(base: &Path, rel: &str, content: &str) {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    #[test]
    fn layer3_resolves_directory_prefix_import() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "packages/shared/ReactFeatureFlags.js",
            "// flags",
        );

        let root = tmp.path().join("packages/shared");
        let resolved_path = tmp
            .path()
            .join("packages/shared/ReactFeatureFlags.js")
            .to_str()
            .unwrap()
            .to_string();

        let mut manifest_paths = HashSet::new();
        manifest_paths.insert(resolved_path.clone());

        let result =
            resolve_by_directory_prefix("shared/ReactFeatureFlags", &[root], &manifest_paths);
        assert_eq!(result.unwrap().to_str().unwrap(), resolved_path);
    }

    #[test]
    fn layer3_returns_none_for_external_packages() {
        let tmp = TempDir::new().unwrap();
        // Directory "react" exists, but "react/something" is NOT in manifest
        fs::create_dir_all(tmp.path().join("packages/react")).unwrap();

        let root = tmp.path().join("packages/react");
        let manifest_paths: HashSet<String> = HashSet::new(); // empty — nothing indexed

        let result = resolve_by_directory_prefix("react/something", &[root], &manifest_paths);
        assert!(result.is_none());
    }

    #[test]
    fn layer3_returns_none_when_no_prefix_match() {
        let roots = vec![PathBuf::from("/repo/packages/shared")];
        let manifest_paths: HashSet<String> = HashSet::new();

        // "lodash/fp" — no workspace root named "lodash"
        let result = resolve_by_directory_prefix("lodash/fp", &roots, &manifest_paths);
        assert!(result.is_none());
    }

    #[test]
    fn layer3_returns_none_for_bare_specifier() {
        // No slash → split_once returns None → function returns None
        let roots = vec![PathBuf::from("/repo/packages/shared")];
        let manifest_paths: HashSet<String> = HashSet::new();
        let result = resolve_by_directory_prefix("bare-package", &roots, &manifest_paths);
        assert!(result.is_none());
    }
}
