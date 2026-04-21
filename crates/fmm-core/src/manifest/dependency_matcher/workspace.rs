use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::manifest::{FileEntry, Manifest};
use crate::resolver::{
    CrossPackageResolver, DenoImportResolver, GoImportResolver, ImportResolver, RustImportResolver,
    resolve_by_directory_prefix, workspace::WorkspaceEcosystem,
};

use super::path::{is_go_source_file, is_js_ts_source_file, is_rust_source_file};

pub fn collect_workspace_edges(manifest: &Manifest) -> Vec<(String, String)> {
    let js_resolver: Arc<dyn ImportResolver> = Arc::new(CrossPackageResolver::new(
        manifest.workspace_packages_for(WorkspaceEcosystem::Js),
    ));
    let deno_resolver = Arc::new(DenoImportResolver::new(
        &manifest.workspace_packages,
        &manifest.workspace_roots,
    ));
    let rust_resolver: Arc<dyn ImportResolver> = Arc::new(RustImportResolver::new(
        manifest.workspace_packages_for(WorkspaceEcosystem::Rust),
    ));
    let go_resolver: Arc<dyn ImportResolver> = Arc::new(GoImportResolver::new(
        manifest.workspace_packages_for(WorkspaceEcosystem::Go),
    ));

    // Build canonical to original key map. On macOS, /var/... symlinks resolve
    // to /private/var/..., so resolver output can differ from manifest keys.
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
                if deno_resolver.is_deno_source(importer) {
                    deno_workspace_edges(
                        file_path,
                        entry,
                        importer,
                        deno_resolver.as_ref(),
                        &original_keys,
                        &canonical_to_original,
                    )
                } else {
                    js_workspace_edges(
                        file_path,
                        entry,
                        importer,
                        manifest.workspace_roots_for(WorkspaceEcosystem::Js),
                        js_resolver.as_ref(),
                        &original_keys,
                        &canonical_to_original,
                    )
                }
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

fn deno_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    resolver: &DenoImportResolver,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<(String, String)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| {
            resolver
                .resolve(importer, import_str)
                .and_then(|resolved| {
                    resolved_to_manifest_key(&resolved, original_keys, canonical_to_original)
                })
                .map(|target_key| (target_key, file_path.to_string()))
        })
        .collect()
}

fn js_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    workspace_roots: &[PathBuf],
    resolver: &dyn ImportResolver,
    original_keys: &HashSet<String>,
    canonical_to_original: &HashMap<String, String>,
) -> Vec<(String, String)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| {
            if let Some(resolved) = resolver.resolve(importer, import_str)
                && let Some(target_key) =
                    resolved_to_manifest_key(&resolved, original_keys, canonical_to_original)
            {
                return Some((target_key, file_path.to_string()));
            }

            resolve_by_directory_prefix(import_str, workspace_roots, original_keys).and_then(
                |path| {
                    resolved_to_manifest_key(&path, original_keys, canonical_to_original)
                        .map(|target_key| (target_key, file_path.to_string()))
                },
            )
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
