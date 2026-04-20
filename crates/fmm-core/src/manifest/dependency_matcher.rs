use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use rayon::prelude::*;

use crate::resolver::{CrossPackageResolver, ImportResolver, resolve_by_directory_prefix};

use super::Manifest;

const JS_TS_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

fn is_js_ts_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| JS_TS_SOURCE_EXTENSIONS.contains(&ext))
}

/// Return a reference to the lazily-initialised set of source-file extensions
/// from the builtin `ParserRegistry`.
///
/// Initialised once on first call; subsequent calls are lock-free reads.
pub(crate) fn builtin_source_extensions() -> &'static HashSet<String> {
    static EXTS: OnceLock<HashSet<String>> = OnceLock::new();
    EXTS.get_or_init(|| {
        let registry = crate::parser::ParserRegistry::with_builtins();
        registry.source_extensions().clone()
    })
}

/// Strip a file extension from `path` when the suffix is a recognised source-file
/// extension. Returns the original string unchanged for compound names like
/// `runtime.exception` or `crypto.utils` where the dot is part of the filename.
///
/// Pass `builtin_source_extensions()` at call sites that do not have a live registry.
pub(crate) fn strip_source_ext<'a>(path: &'a str, known_extensions: &HashSet<String>) -> &'a str {
    if let Some((stem, ext)) = path.rsplit_once('.') {
        if known_extensions.contains(ext) {
            stem
        } else {
            path
        }
    } else {
        path
    }
}

/// Check if a dependency path from `dependent_file` resolves to `target_file`.
/// Dependencies are stored as relative paths like "../utils/crypto.utils.js"
/// and need to be resolved against the dependent file's directory.
///
/// `known_extensions` is the set of recognised source-file extensions (without
/// the leading dot), typically from `ParserRegistry::source_extensions()`.
pub fn dep_matches(
    dep: &str,
    target_file: &str,
    dependent_file: &str,
    known_extensions: &HashSet<String>,
) -> bool {
    // Resolve the dependency path relative to the dependent file's directory
    let dep_dir = dependent_file
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");

    // Build resolved path by applying relative segments
    let mut parts: Vec<&str> = if dep_dir.is_empty() {
        Vec::new()
    } else {
        dep_dir.split('/').collect()
    };

    let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
    for segment in dep_clean.split('/') {
        if segment == ".." {
            parts.pop();
        } else if segment != "." {
            parts.push(segment);
        }
    }

    let resolved = parts.join("/");

    // Strip extension from the target file path (always has a real extension like .ts/.js).
    // For the resolved dep, only strip if the suffix is a known source-file extension —
    // NestJS-style compound names like `runtime.exception` use `.exception` as part of the
    // filename, not as an extension, and stripping it would produce a wrong stem.
    let resolved_stem = strip_source_ext(&resolved, known_extensions);
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(target_file);

    if resolved_stem == target_stem {
        return true;
    }

    // Python packages: `./utils` should match `utils/__init__.py`
    if let Some(package_stem) = target_stem.strip_suffix("/__init__")
        && resolved_stem == package_stem
    {
        return true;
    }

    // Fallback: crate:: paths (Rust internal modules)
    // e.g. "crate::config" matches "src/config.rs"
    if let Some(module_path_str) = dep.strip_prefix("crate::") {
        let module_path = module_path_str.replace("::", "/");
        return target_stem.ends_with(&module_path);
    }

    // Fallback: domain-qualified paths (Go module paths, etc.)
    // e.g. "github.com/user/project/internal/handler" matches "internal/handler/handler.go"
    // Try progressively shorter path suffixes until one matches.
    if dep.contains('/') && !dep.starts_with('.') {
        let segments: Vec<&str> = dep.split('/').collect();
        for start in 1..segments.len() {
            let suffix = segments[start..].join("/");
            if target_stem.ends_with(&suffix) {
                return true;
            }
        }
    }

    false
}

fn resolve_python_relative_path(dep: &str, source_file: &str) -> Option<String> {
    debug_assert!(dep.starts_with('.') && !dep.starts_with("./"));
    let dots = dep.chars().take_while(|&c| c == '.').count();
    let module_name = &dep[dots..];

    let source_dir = source_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = if source_dir.is_empty() {
        vec![]
    } else {
        source_dir.split('/').collect()
    };

    // Single dot = current package; each additional dot = one level up
    for _ in 1..dots {
        parts.pop()?; // None if we'd go above the root
    }

    if module_name.is_empty() {
        // `from . import X` — no module name, can't pinpoint a file
        return None;
    }

    for part in module_name.split('.') {
        parts.push(part);
    }

    Some(parts.join("/"))
}

/// Match a Python-style relative import (`._run`, `..utils`) against a target
/// file path, given the dependent file's location. Used for downstream detection.
pub fn python_dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
    if !dep.starts_with('.') || dep.starts_with("./") || dep.starts_with("../") {
        return false;
    }
    if let Some(resolved) = resolve_python_relative_path(dep, dependent_file) {
        let target_stem = target_file
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(target_file);
        resolved == target_stem
    } else {
        false
    }
}

/// Match a Python absolute module import (`agno.models.message`) against a target
/// file path. Used for downstream detection.
///
/// Returns true when the dotted path resolves to the target file, considering
/// both root-relative paths (`agno/models/message.py`) and src-layout paths
/// (`src/agno/models/message.py`).
pub fn dotted_dep_matches(dep: &str, target_file: &str) -> bool {
    // Only handle dotted absolute imports — exclude relative (`.X`), paths (`/`), Rust (`::`)
    if dep.starts_with('.') || dep.contains('/') || dep.contains("::") || !dep.contains('.') {
        return false;
    }
    let path_stem = dep.replace('.', "/");
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(target_file);
    // Handle packages: `agno.models` resolves to `agno/models/__init__.py`
    let effective = target_stem.strip_suffix("/__init__").unwrap_or(target_stem);
    effective == path_stem.as_str() || effective.ends_with(&format!("/{}", path_stem))
}

/// Attempt to resolve a dependency string to a file path present in the manifest.
///
/// Handles Python-style relative imports (`._run`, `..config`), JS/TS-style relative
/// paths (`./utils`, `../config`), Go module paths, and Rust `crate::` paths.
///
/// `known_extensions` is the set of recognised source-file extensions (without
/// the leading dot), typically from `ParserRegistry::source_extensions()`.
pub(crate) fn try_resolve_local_dep(
    dep: &str,
    source_file: &str,
    manifest: &Manifest,
    known_extensions: &HashSet<String>,
) -> Option<String> {
    // Python-style relative imports: start with . but NOT ./ or ../
    if dep.starts_with('.') && !dep.starts_with("./") && !dep.starts_with("../") {
        let resolved_stem = resolve_python_relative_path(dep, source_file)?;
        for candidate in [
            format!("{}.py", resolved_stem),
            format!("{}/__init__.py", resolved_stem),
            resolved_stem.clone(),
        ] {
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return None;
    }
    // JS/TS-style or other relative paths: use dep_matches to find the manifest key
    if dep.starts_with("./") || dep.starts_with("../") {
        // First: direct file match (extension-agnostic stem comparison)
        if let Some(found) = manifest
            .files
            .keys()
            .find(|path| dep_matches(dep, path, source_file, known_extensions))
        {
            return Some(found.clone());
        }
        // Fallback: directory-style JS/TS imports — `./module` should match `module/index.ts`
        // when no direct file `module.ts` exists. Direct match takes priority above.
        let dep_dir = source_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let mut parts: Vec<&str> = if dep_dir.is_empty() {
            Vec::new()
        } else {
            dep_dir.split('/').collect()
        };
        let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
        for segment in dep_clean.split('/') {
            match segment {
                ".." => {
                    parts.pop();
                }
                "." => {}
                s => parts.push(s),
            }
        }
        let resolved = parts.join("/");
        for index_name in &["index.ts", "index.tsx", "index.js", "index.jsx"] {
            let candidate = format!("{}/{}", resolved, index_name);
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return None;
    }
    // Domain-qualified paths: Go module paths (github.com/...) and Rust crate:: paths.
    // dep_matches has suffix-matching fallback for these. Plain external packages like
    // "anyhow" or "fmt" (no "/" or "::") are left as external.
    if dep.contains('/') || dep.contains("::") {
        return manifest
            .files
            .keys()
            .find(|path| dep_matches(dep, path, source_file, known_extensions))
            .cloned();
    }
    // Dotted module path: Python absolute self-imports (e.g. `agno.models.message`).
    // Replace dots with slashes and suffix-match against manifest file stems.
    if dep.contains('.') {
        let path_stem = dep.replace('.', "/");
        for candidate in [
            format!("{}.py", path_stem),
            format!("{}/__init__.py", path_stem),
            path_stem.clone(),
        ] {
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return manifest
            .files
            .keys()
            .find(|path| {
                let stem = path.rsplit_once('.').map(|(s, _)| s).unwrap_or(path);
                let effective = stem.strip_suffix("/__init__").unwrap_or(stem);
                effective == path_stem.as_str() || effective.ends_with(&format!("/{}", path_stem))
            })
            .cloned();
    }
    None
}

/// Build the reverse dependency index from a fully-loaded manifest.
///
/// Maps each file to the set of files that directly import it. Built once at
/// manifest load time so downstream lookups are O(1) instead of O(N × D).
///
/// Three resolution paths:
/// - `entry.dependencies`: relative imports resolved via `try_resolve_local_dep` /
///   `dep_matches` / `python_dep_matches`
/// - `entry.imports` (Python dotted style): resolved via `dotted_dep_matches`
/// - `entry.imports` (JS/TS cross-package bare specifiers): resolved via the
///   three-layer `CrossPackageResolver` (tsconfig paths + workspace aliases + directory
///   prefix heuristic)
pub(crate) fn build_reverse_deps(manifest: &Manifest) -> HashMap<String, Vec<String>> {
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
    let exts = builtin_source_extensions();

    // Pass 1: relative and non-relative dependencies (unchanged behavior)
    for (source, entry) in &manifest.files {
        for dep in &entry.dependencies {
            if dep.starts_with("./") || dep.starts_with("../") {
                // Relative: try_resolve_local_dep gives the single canonical target
                if let Some(target) = try_resolve_local_dep(dep, source, manifest, exts) {
                    rev.entry(target).or_default().push(source.clone());
                }
            } else {
                // Non-relative: mirror dep_matches + python_dep_matches (same as dep_targets_file)
                for target in manifest.files.keys() {
                    if dep_matches(dep, target, source, exts)
                        || python_dep_matches(dep, target, source)
                    {
                        rev.entry(target.clone()).or_default().push(source.clone());
                    }
                }
            }
        }

        // Pass 2a: Python dotted absolute imports (existing behavior preserved)
        for imp in &entry.imports {
            for target in manifest.files.keys() {
                if dotted_dep_matches(imp, target) {
                    rev.entry(target.clone()).or_default().push(source.clone());
                }
            }
        }
    }

    // Pass 2b: language-dispatched cross-package bare specifiers.
    // JS/TS uses the three-layer resolver. Other languages skip here until
    // their ecosystem-specific resolvers plug into ImportResolver.
    // Runs in parallel via Rayon; CrossPackageResolver is Send+Sync via Arc<Resolver>.
    // Skipped if no workspace config was found (workspace_packages and workspace_roots both empty).
    if !manifest.workspace_packages.is_empty() || !manifest.workspace_roots.is_empty() {
        let resolver: std::sync::Arc<dyn ImportResolver> =
            std::sync::Arc::new(CrossPackageResolver::new(&manifest.workspace_packages));

        // Build canonical → original key map. On macOS, /var/... symlinks resolve to
        // /private/var/..., so the resolver (symlinks=true) returns canonical paths while
        // manifest keys are the original paths from sidecar file: fields. This map lets us
        // match resolver output back to the correct original manifest key.
        let canonical_to_original: std::sync::Arc<HashMap<String, String>> = std::sync::Arc::new(
            manifest
                .files
                .keys()
                .filter_map(|k| {
                    let canonical = std::fs::canonicalize(k).ok()?;
                    Some((canonical.to_str()?.to_string(), k.clone()))
                })
                .collect(),
        );

        // Also keep the original keys set for Layer 3 (which constructs paths from
        // workspace_roots — also non-canonical — so we can do direct lookup there).
        let original_keys: std::sync::Arc<HashSet<String>> =
            std::sync::Arc::new(manifest.files.keys().cloned().collect());

        let cross_package_edges: Vec<(String, String)> = manifest
            .files
            .par_iter()
            .flat_map(|(file_path, entry)| {
                let importer = Path::new(file_path.as_str());
                if !is_js_ts_source_file(importer) {
                    return Vec::new();
                }

                let resolver = std::sync::Arc::clone(&resolver);
                let canonical_to_original = std::sync::Arc::clone(&canonical_to_original);
                let original_keys = std::sync::Arc::clone(&original_keys);

                entry
                    .imports
                    .iter()
                    .filter_map(|import_str| {
                        // Skip Python-style dotted imports — already handled in pass 2a
                        if import_str.contains('.') && !import_str.contains('/') {
                            return None;
                        }

                        // Layer 3 produces paths via workspace_roots (non-canonical), so
                        // it uses original_keys for its manifest guard.
                        let layer3_result = || {
                            resolve_by_directory_prefix(
                                import_str,
                                &manifest.workspace_roots,
                                &original_keys,
                            )
                        };

                        // Layer 1 + 2: oxc-resolver with tsconfig paths and workspace aliases
                        if let Some(resolved) = resolver.resolve(importer, import_str) {
                            // Resolver may return canonical path (symlinks=true) — map back
                            // to the original manifest key via canonical_to_original.
                            let original_key = resolved.to_str().and_then(|s| {
                                if original_keys.contains(s) {
                                    Some(s.to_string())
                                } else {
                                    // Try canonical lookup
                                    std::fs::canonicalize(&resolved)
                                        .ok()
                                        .and_then(|c| c.to_str().map(|s| s.to_string()))
                                        .and_then(|c| canonical_to_original.get(&c).cloned())
                                }
                            });
                            if let Some(target_key) = original_key {
                                return Some((target_key, file_path.clone()));
                            }
                        }

                        // Layer 3 fallback: directory prefix heuristic
                        layer3_result().and_then(|path| {
                            let path_str = path.to_str()?.to_string();
                            if original_keys.contains(&path_str) {
                                Some((path_str, file_path.clone()))
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Merge cross-package edges into reverse index
        for (target, importer) in cross_package_edges {
            rev.entry(target).or_default().push(importer);
        }
    }

    for v in rev.values_mut() {
        v.sort();
        v.dedup();
    }
    rev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exts() -> &'static HashSet<String> {
        builtin_source_extensions()
    }

    #[test]
    fn build_reverse_deps_dispatches_cross_package_resolution_by_source_extension() {
        use std::fs;

        let tmp = tempfile::TempDir::new().unwrap();
        let shared_dir = tmp.path().join("packages/shared");
        let target = shared_dir.join("util.ts");
        let ts_importer = tmp.path().join("packages/app/index.ts");
        let rs_importer = tmp.path().join("crates/app/src/lib.rs");

        for path in [&target, &ts_importer, &rs_importer] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }

        let target_key = target.to_string_lossy().into_owned();
        let ts_importer_key = ts_importer.to_string_lossy().into_owned();
        let rs_importer_key = rs_importer.to_string_lossy().into_owned();

        let mut manifest = Manifest::new();
        manifest.workspace_roots.push(shared_dir);
        manifest
            .files
            .insert(target_key.clone(), crate::manifest::FileEntry::default());
        manifest.files.insert(
            ts_importer_key.clone(),
            crate::manifest::FileEntry {
                imports: vec!["shared/util".to_string()],
                ..Default::default()
            },
        );
        manifest.files.insert(
            rs_importer_key.clone(),
            crate::manifest::FileEntry {
                imports: vec!["shared/util".to_string()],
                ..Default::default()
            },
        );

        let reverse_deps = build_reverse_deps(&manifest);
        let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

        assert!(
            importers.contains(&ts_importer_key),
            "TS importer should resolve through the JS/TS cross-package path, got: {:?}",
            importers
        );
        assert!(
            !importers.contains(&rs_importer_key),
            "Rust importer should wait for RustImportResolver, got: {:?}",
            importers
        );
    }

    #[test]
    fn dep_matches_relative_path() {
        // dep "./types" from "src/index.ts" resolves to "src/types"
        assert!(dep_matches(
            "./types",
            "src/types.ts",
            "src/index.ts",
            exts()
        ));
        assert!(dep_matches(
            "./config",
            "src/config.ts",
            "src/index.ts",
            exts()
        ));
        assert!(!dep_matches(
            "./types",
            "src/other.ts",
            "src/index.ts",
            exts()
        ));
    }

    #[test]
    fn dep_matches_compound_filename_with_dot() {
        // NestJS convention: files named `foo.exception.ts`, `foo.service.ts` etc.
        // The dep stored without extension is `../errors/runtime.exception` — the `.exception`
        // part is the filename segment, not a file extension, and must not be stripped.
        assert!(dep_matches(
            "../errors/exceptions/runtime.exception",
            "packages/core/errors/exceptions/runtime.exception.ts",
            "packages/core/injector/injector.ts",
            exts(),
        ));
        assert!(dep_matches(
            "../errors/exceptions/undefined-dependency.exception",
            "packages/core/errors/exceptions/undefined-dependency.exception.ts",
            "packages/core/injector/injector.ts",
            exts(),
        ));
        // Regular dep with an actual .js extension should still resolve to .ts
        assert!(dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/services/auth.service.ts",
            exts(),
        ));
    }

    #[test]
    fn dep_matches_nested_path() {
        // dep "./utils/helpers" from "src/index.ts" resolves to "src/utils/helpers"
        assert!(dep_matches(
            "./utils/helpers",
            "src/utils/helpers.ts",
            "src/index.ts",
            exts(),
        ));
        assert!(!dep_matches(
            "./utils/helpers",
            "src/utils/other.ts",
            "src/index.ts",
            exts(),
        ));
    }

    #[test]
    fn dep_matches_parent_relative() {
        // dep "../utils/crypto.utils.js" from "pkg/src/services/auth.service.ts"
        // resolves to "pkg/src/utils/crypto.utils"
        assert!(dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/services/auth.service.ts",
            exts(),
        ));
        assert!(!dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/services/other.ts",
            "pkg/src/services/auth.service.ts",
            exts(),
        ));
    }

    #[test]
    fn dep_matches_deep_parent_relative() {
        // dep "../../../utils/crypto.utils.js" from "pkg/src/tests/unit/auth/test.ts"
        // resolves to "pkg/src/utils/crypto.utils" (going up 3 dirs from tests/unit/auth)
        assert!(dep_matches(
            "../../../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/tests/unit/auth/test.ts",
            exts(),
        ));
    }

    #[test]
    fn dep_matches_without_prefix() {
        assert!(dep_matches("types", "src/types.ts", "src/index.ts", exts()));
    }

    #[test]
    fn dep_matches_python_package() {
        // `./utils` should resolve to `utils/__init__.py` (Python package)
        assert!(dep_matches(
            "./utils",
            "src/utils/__init__.py",
            "src/service.py",
            exts(),
        ));
        // `../models` should resolve to `models/__init__.py` one level up
        assert!(dep_matches(
            "../models",
            "models/__init__.py",
            "src/service.py",
            exts(),
        ));
        // Should still match plain module file
        assert!(dep_matches(
            "./utils",
            "src/utils.py",
            "src/service.py",
            exts()
        ));
        // No false positive: different package
        assert!(!dep_matches(
            "./utils",
            "src/auth/__init__.py",
            "src/service.py",
            exts(),
        ));
    }

    #[test]
    fn dep_matches_crate_path() {
        // Rust crate:: paths resolve via suffix matching
        assert!(dep_matches(
            "crate::config",
            "src/config.rs",
            "src/main.rs",
            exts()
        ));
        assert!(dep_matches(
            "crate::parser::builtin",
            "src/parser/builtin.rs",
            "src/main.rs",
            exts(),
        ));
        // No false positives
        assert!(!dep_matches(
            "crate::config",
            "src/other.rs",
            "src/main.rs",
            exts()
        ));
    }

    #[test]
    fn dep_matches_go_module_path() {
        // Go domain-qualified module paths resolve via suffix matching
        assert!(dep_matches(
            "github.com/user/project/internal/handler",
            "internal/handler/handler.go",
            "cmd/main.go",
            exts(),
        ));
        // Stdlib short paths don't match unrelated files
        assert!(!dep_matches(
            "fmt",
            "internal/format/format.go",
            "cmd/main.go",
            exts(),
        ));
    }
}
