use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::identity::EdgeKind;
use crate::manifest::{FileEntry, Manifest};
use crate::resolver::{
    CrossPackageResolver, DenoImportResolver, GoImportResolver, ImportResolver, RustImportResolver,
    normal_components, resolve_by_directory_prefix, workspace::WorkspaceEcosystem,
};

use super::dependency_kind;
use super::path::{is_go_source_file, is_js_ts_source_file, is_rust_source_file};

pub(super) fn collect_workspace_edges(manifest: &Manifest) -> Vec<(String, String, EdgeKind)> {
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

    let original_keys: HashSet<String> = manifest.files.keys().cloned().collect();
    let manifest_roots = infer_manifest_roots(manifest);
    // Build canonical to original key map. On macOS, /var/... symlinks resolve
    // to /private/var/..., so resolver output can differ from manifest keys.
    let canonical_to_original = canonical_manifest_keys(&original_keys, &manifest_roots);
    let key_map = WorkspaceKeyMap {
        original_keys: &original_keys,
        manifest_roots: &manifest_roots,
        canonical_to_original: &canonical_to_original,
    };

    manifest
        .files
        .par_iter()
        .flat_map(|(file_path, entry)| {
            let importer_path = absolute_manifest_path(file_path, &manifest_roots);
            let importer = importer_path.as_path();
            if is_js_ts_source_file(importer) {
                if deno_resolver.is_deno_source(importer) {
                    deno_workspace_edges(
                        file_path,
                        entry,
                        importer,
                        deno_resolver.as_ref(),
                        key_map,
                    )
                } else {
                    js_workspace_edges(
                        file_path,
                        entry,
                        importer,
                        manifest.workspace_roots_for(WorkspaceEcosystem::Js),
                        js_resolver.as_ref(),
                        key_map,
                    )
                }
            } else if is_rust_source_file(importer) {
                rust_workspace_edges(file_path, entry, importer, rust_resolver.as_ref(), key_map)
            } else if is_go_source_file(importer) {
                go_workspace_edges(file_path, entry, importer, go_resolver.as_ref(), key_map)
            } else {
                Vec::new()
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
struct WorkspaceKeyMap<'a> {
    original_keys: &'a HashSet<String>,
    manifest_roots: &'a [PathBuf],
    canonical_to_original: &'a HashMap<String, String>,
}

fn resolved_to_manifest_key(resolved: &Path, key_map: WorkspaceKeyMap<'_>) -> Option<String> {
    let resolved_str = resolved.to_str()?;
    if key_map.original_keys.contains(resolved_str) {
        return Some(resolved_str.to_string());
    }

    for root in key_map.manifest_roots {
        if let Ok(relative) = resolved.strip_prefix(root)
            && let Some(key) = slash_path(relative)
            && key_map.original_keys.contains(&key)
        {
            return Some(key);
        }
    }

    std::fs::canonicalize(resolved)
        .ok()
        .and_then(|canonical| canonical.to_str().map(str::to_string))
        .and_then(|canonical| key_map.canonical_to_original.get(&canonical).cloned())
}

fn canonical_manifest_keys(
    original_keys: &HashSet<String>,
    manifest_roots: &[PathBuf],
) -> HashMap<String, String> {
    let mut canonical_to_original = HashMap::new();
    for key in original_keys {
        for path in candidate_manifest_paths(key, manifest_roots) {
            if let Ok(canonical) = std::fs::canonicalize(path)
                && let Some(canonical) = canonical.to_str()
            {
                canonical_to_original.insert(canonical.to_string(), key.clone());
            }
        }
    }
    canonical_to_original
}

fn candidate_manifest_paths(key: &str, manifest_roots: &[PathBuf]) -> Vec<PathBuf> {
    let path = PathBuf::from(key);
    if path.is_absolute() {
        return vec![path];
    }

    let mut paths = vec![path];
    paths.extend(manifest_roots.iter().map(|root| root.join(key)));
    paths
}

fn absolute_manifest_path(key: &str, manifest_roots: &[PathBuf]) -> PathBuf {
    let path = PathBuf::from(key);
    if path.is_absolute() {
        return path;
    }

    manifest_roots
        .iter()
        .map(|root| root.join(key))
        .find(|candidate| candidate.exists())
        .unwrap_or(path)
}

fn infer_manifest_roots(manifest: &Manifest) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let package_dirs = manifest
        .workspace_packages
        .values()
        .chain(manifest.workspace_roots.iter());

    for package_dir in package_dirs {
        for key in manifest.files.keys() {
            if let Some(root) = infer_root_for_key(package_dir, key) {
                roots.push(root);
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

fn infer_root_for_key(package_dir: &Path, key: &str) -> Option<PathBuf> {
    if Path::new(key).is_absolute() {
        return None;
    }

    let package_components = normal_components(package_dir);
    let key_components = normal_components(Path::new(key));
    let max = package_components.len().min(key_components.len());

    for len in (1..=max).rev() {
        if package_components[package_components.len() - len..] == key_components[..len] {
            let mut root = package_dir.to_path_buf();
            for _ in 0..len {
                root = root.parent()?.to_path_buf();
            }
            return Some(root);
        }
    }

    None
}

fn slash_path(path: &Path) -> Option<String> {
    let parts = path
        .components()
        .map(|component| match component {
            std::path::Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(parts.join("/"))
}

fn deno_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    resolver: &DenoImportResolver,
    key_map: WorkspaceKeyMap<'_>,
) -> Vec<(String, String, EdgeKind)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| {
            resolver
                .resolve(importer, import_str)
                .and_then(|resolved| resolved_to_manifest_key(&resolved, key_map))
                .map(|target_key| {
                    (
                        target_key,
                        file_path.to_string(),
                        dependency_kind(entry, import_str),
                    )
                })
        })
        .collect()
}

fn js_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    workspace_roots: &[PathBuf],
    resolver: &dyn ImportResolver,
    key_map: WorkspaceKeyMap<'_>,
) -> Vec<(String, String, EdgeKind)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| {
            let kind = dependency_kind(entry, import_str);
            if let Some(resolved) = resolver.resolve(importer, import_str)
                && let Some(target_key) = resolved_to_manifest_key(&resolved, key_map)
            {
                return Some((target_key, file_path.to_string(), kind));
            }

            resolve_by_directory_prefix(import_str, workspace_roots, key_map.original_keys)
                .and_then(|path| {
                    resolved_to_manifest_key(&path, key_map)
                        .map(|target_key| (target_key, file_path.to_string(), kind))
                })
        })
        .collect()
}

fn rust_workspace_edges(
    file_path: &str,
    entry: &FileEntry,
    importer: &Path,
    resolver: &dyn ImportResolver,
    key_map: WorkspaceKeyMap<'_>,
) -> Vec<(String, String, EdgeKind)> {
    rust_import_specifiers(entry)
        .into_iter()
        .filter_map(|specifier| {
            resolver
                .resolve(importer, &specifier)
                .and_then(|resolved| resolved_to_manifest_key(&resolved, key_map))
                .map(|target_key| (target_key, file_path.to_string(), EdgeKind::Runtime))
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
    key_map: WorkspaceKeyMap<'_>,
) -> Vec<(String, String, EdgeKind)> {
    entry
        .imports
        .iter()
        .filter_map(|import_str| resolver.resolve(importer, import_str))
        .flat_map(|package_dir| {
            go_package_manifest_keys(&package_dir, key_map)
                .into_iter()
                .map(|target_key| (target_key, file_path.to_string(), EdgeKind::Runtime))
        })
        .collect()
}

fn go_package_manifest_keys(package_dir: &Path, key_map: WorkspaceKeyMap<'_>) -> Vec<String> {
    let mut keys: Vec<String> = key_map
        .original_keys
        .iter()
        .filter(|key| {
            is_go_package_file(key, package_dir)
                || key_map
                    .manifest_roots
                    .iter()
                    .any(|root| is_go_package_file(&root.join(key).to_string_lossy(), package_dir))
        })
        .cloned()
        .collect();

    if keys.is_empty()
        && let Ok(canonical_package_dir) = std::fs::canonicalize(package_dir)
    {
        keys = key_map
            .canonical_to_original
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
