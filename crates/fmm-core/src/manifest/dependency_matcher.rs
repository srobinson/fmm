use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use rayon::prelude::*;

use crate::resolver::{
    CrossPackageResolver, GoImportResolver, ImportResolver, RustImportResolver,
    resolve_by_directory_prefix,
};

use super::{FileEntry, Manifest};

const JS_TS_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
const RUST_SOURCE_EXTENSIONS: &[&str] = &["rs"];
const GO_SOURCE_EXTENSIONS: &[&str] = &["go"];

fn is_js_ts_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| JS_TS_SOURCE_EXTENSIONS.contains(&ext))
}

fn is_rust_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RUST_SOURCE_EXTENSIONS.contains(&ext))
}

fn is_go_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| GO_SOURCE_EXTENSIONS.contains(&ext))
}

fn is_cargo_workspace_source(path: &Path, manifest: &Manifest) -> bool {
    is_rust_source_file(path)
        && manifest
            .workspace_packages
            .values()
            .any(|dir| path.starts_with(dir) && dir.join("Cargo.toml").exists())
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
    // For the resolved dep, only strip if the suffix is a known source-file extension,
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
        // `from . import X`, no module name, can't pinpoint a file
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
    // Only handle dotted absolute imports. Exclude relative (`.X`), paths (`/`), Rust (`::`).
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
        // Fallback: directory-style JS/TS imports. `./module` should match `module/index.ts`
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

fn resolved_to_manifest_key(
    resolved: &Path,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Option<String> {
    let resolved_str = resolved.to_str()?;
    if original_keys.contains(resolved_str) {
        return Some(resolved_str.to_string());
    }

    std::fs::canonicalize(resolved)
        .ok()
        .and_then(|canonical| canonical.to_str().map(str::to_string))
        .and_then(|canonical| canonical_to_original.get(&canonical).cloned())
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
/// - workspace imports: resolved by language dispatch. JS/TS uses
///   `CrossPackageResolver`; Rust uses `RustImportResolver`; Go uses
///   `GoImportResolver`.
pub(crate) fn build_reverse_deps(manifest: &Manifest) -> HashMap<String, Vec<String>> {
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
    let exts = builtin_source_extensions();
    let has_workspace =
        !manifest.workspace_packages.is_empty() || !manifest.workspace_roots.is_empty();

    // Pass 1: relative and non-relative dependencies (unchanged behavior)
    for (source, entry) in &manifest.files {
        let source_path = Path::new(source.as_str());
        let rust_workspace_source =
            has_workspace && is_cargo_workspace_source(source_path, manifest);
        for dep in &entry.dependencies {
            if rust_workspace_source
                && (dep.starts_with("./") || dep.starts_with("../") || dep.starts_with("crate::"))
            {
                // Rust local paths need module semantics. Generic relative
                // matching can misread `super::foo` as a filesystem `../foo`.
                continue;
            } else if dep.starts_with("./") || dep.starts_with("../") {
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

    // Pass 2b: language-dispatched workspace specifiers.
    // JS/TS uses the three-layer resolver. Rust uses Cargo module semantics
    // for cross-crate and crate-local paths. Go uses longest-prefix module
    // matching and maps package directories to indexed .go files.
    // Runs in parallel via Rayon; resolvers are Send+Sync through Arc.
    // Skipped if no workspace config was found (workspace_packages and workspace_roots both empty).
    if has_workspace {
        for (target, importer) in collect_workspace_edges(manifest) {
            rev.entry(target).or_default().push(importer);
        }
    }

    for v in rev.values_mut() {
        v.sort();
        v.dedup();
    }
    rev
}

fn collect_workspace_edges(manifest: &Manifest) -> Vec<(String, String)> {
    let js_resolver: std::sync::Arc<dyn ImportResolver> =
        std::sync::Arc::new(CrossPackageResolver::new(&manifest.workspace_packages));
    let rust_resolver: std::sync::Arc<dyn ImportResolver> =
        std::sync::Arc::new(RustImportResolver::new(&manifest.workspace_packages));
    let go_resolver: std::sync::Arc<dyn ImportResolver> =
        std::sync::Arc::new(GoImportResolver::new(&manifest.workspace_packages));

    // Build canonical -> original key map. On macOS, /var/... symlinks resolve to
    // /private/var/..., so resolver output can differ from manifest keys.
    let canonical_to_original: HashMap<String, String> = manifest
        .files
        .keys()
        .filter_map(|k| {
            let canonical = std::fs::canonicalize(k).ok()?;
            Some((canonical.to_str()?.to_string(), k.clone()))
        })
        .collect();
    let original_keys: HashSet<String> = manifest.files.keys().cloned().collect();

    manifest
        .files
        .par_iter()
        .flat_map(|(file_path, entry)| {
            let importer = Path::new(file_path.as_str());
            if is_js_ts_source_file(importer) {
                js_workspace_edges(
                    file_path,
                    entry,
                    importer,
                    manifest,
                    js_resolver.as_ref(),
                    &original_keys,
                    &canonical_to_original,
                )
            } else if is_rust_source_file(importer) {
                rust_workspace_edges(
                    file_path,
                    entry,
                    importer,
                    rust_resolver.as_ref(),
                    &original_keys,
                    &canonical_to_original,
                )
            } else if is_go_source_file(importer) {
                go_workspace_edges(
                    file_path,
                    entry,
                    importer,
                    go_resolver.as_ref(),
                    &original_keys,
                    &canonical_to_original,
                )
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn js_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    manifest: &Manifest,
    resolver: &dyn ImportResolver,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<(String, String)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| {
            if import_str.contains('.') && !import_str.contains('/') {
                return None;
            }

            if let Some(resolved) = resolver.resolve(importer, import_str)
                && let Some(target_key) =
                    resolved_to_manifest_key(&resolved, original_keys, canonical_to_original)
            {
                return Some((target_key, file_path.to_string()));
            }

            resolve_by_directory_prefix(import_str, &manifest.workspace_roots, original_keys)
                .and_then(|path| {
                    resolved_to_manifest_key(&path, original_keys, canonical_to_original)
                        .map(|target_key| (target_key, file_path.to_string()))
                })
        })
        .collect()
}

fn rust_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    resolver: &dyn ImportResolver,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<(String, String)> {
    rust_import_specifiers(entry)
        .into_iter()
        .filter_map(|specifier| {
            resolver
                .resolve(importer, &specifier)
                .and_then(|resolved| {
                    resolved_to_manifest_key(&resolved, original_keys, canonical_to_original)
                })
                .map(|target_key| (target_key, file_path.to_string()))
        })
        .collect()
}

fn rust_import_specifiers(entry: &FileEntry) -> Vec<String> {
    let mut specifiers: Vec<String> = entry
        .dependencies
        .iter()
        .filter(|dep| !dep.starts_with("./") && !dep.starts_with("../"))
        .cloned()
        .collect();
    for (path, names) in &entry.named_imports {
        specifiers.push(path.clone());
        specifiers.extend(names.iter().map(|name| format!("{path}::{name}")));
    }
    specifiers.extend(entry.namespace_imports.clone());
    specifiers.sort();
    specifiers.dedup();
    specifiers
}

fn go_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    resolver: &dyn ImportResolver,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<(String, String)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| resolver.resolve(importer, import_str))
        .flat_map(|package_dir| {
            go_package_manifest_keys(&package_dir, original_keys, canonical_to_original)
                .into_iter()
                .map(|target_key| (target_key, file_path.to_string()))
        })
        .collect()
}

fn go_package_manifest_keys(
    package_dir: &Path,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<String> {
    let mut keys: Vec<String> = original_keys
        .iter()
        .filter(|key| is_go_package_file(key, package_dir))
        .cloned()
        .collect();

    if keys.is_empty()
        && let Ok(canonical_package_dir) = std::fs::canonicalize(package_dir)
    {
        keys = canonical_to_original
            .iter()
            .filter(|(canonical, _)| is_go_package_file(canonical, &canonical_package_dir))
            .map(|(_, original)| original.clone())
            .collect();
    }

    keys.sort();
    keys.dedup();
    keys
}

fn is_go_package_file(file_path: &str, package_dir: &Path) -> bool {
    let path = Path::new(file_path);
    path.parent() == Some(package_dir)
        && path.extension().and_then(|ext| ext.to_str()) == Some("go")
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.ends_with("_test.go"))
}

#[cfg(test)]
#[path = "dependency_matcher_go_tests.rs"]
mod go_tests;

#[cfg(test)]
#[path = "dependency_matcher_tests.rs"]
mod tests;
