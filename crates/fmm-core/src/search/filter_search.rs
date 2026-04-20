use std::collections::{HashSet, VecDeque};

use crate::manifest::{
    Manifest, builtin_source_extensions, dep_matches, dotted_dep_matches, python_dep_matches,
    strip_source_ext, try_resolve_local_dep,
};

use super::helpers::file_entry_to_result;
use super::{FileSearchResult, SearchFilters};

/// Structured filter search: export, imports, depends_on, LOC range.
///
/// All active filters are combined with AND semantics. Results must satisfy every
/// specified constraint. Previously filters used OR (union) semantics which caused
/// silent result pollution when multiple filters were combined (ALP-823).
pub fn filter_search(manifest: &Manifest, filters: &SearchFilters) -> Vec<FileSearchResult> {
    let has_export = filters.export.is_some();
    let has_imports = filters.imports.is_some();
    let has_depends_on = filters.depends_on.is_some();
    let has_loc = filters.min_loc.is_some() || filters.max_loc.is_some();

    // No filters at all -> return everything.
    if !has_export && !has_imports && !has_depends_on && !has_loc {
        let mut results: Vec<FileSearchResult> = manifest
            .files
            .iter()
            .map(|(path, entry)| file_entry_to_result(path, entry))
            .collect();
        results.sort_by(|a, b| a.file.cmp(&b.file));
        return results;
    }

    // Start with all files; each active filter narrows with retain (AND semantics).
    let mut file_set: Vec<(&String, &crate::manifest::FileEntry)> = manifest.files.iter().collect();

    // Export filter: exact O(1) first, then case-insensitive substring fallback.
    if let Some(ref export) = filters.export {
        if let Some(file_path) = manifest.export_index.get(export.as_str()) {
            file_set.retain(|(f, _)| *f == file_path);
        } else {
            let export_lower = export.to_lowercase();
            let matching: HashSet<&String> = manifest
                .export_index
                .iter()
                .filter(|(n, _)| n.to_lowercase().contains(&export_lower))
                .map(|(_, fp)| fp)
                .collect();
            file_set.retain(|(f, _)| matching.contains(*f));
        }
    }

    // Imports filter: file must import the given package/module name.
    // External packages live in entry.imports; local file paths live in entry.dependencies.
    // When the query looks like a local path (contains '/' but not '://'), also check
    // dependencies using the same resolution logic as depends_on so that
    // `imports: src/db/client` works even though deps are stored as relative paths.
    if let Some(ref import_name) = filters.imports {
        let looks_like_local = import_name.contains('/') && !import_name.contains("://");
        let exts = builtin_source_extensions();
        let dep_stem = strip_source_ext(import_name, exts);
        file_set.retain(|(file_path, entry)| {
            let in_imports = entry
                .imports
                .iter()
                .any(|i| i.contains(import_name.as_str()));
            let in_deps = looks_like_local
                && entry.dependencies.iter().any(|d| {
                    dep_targets_file(d, import_name, file_path, manifest, exts)
                        || d.contains(dep_stem)
                });
            in_imports || in_deps
        });
    }

    // depends_on filter: use the prebuilt reverse dependency graph so language
    // resolver edges, including Rust cross-crate paths, participate in search.
    // Fall back to per-file dependency scanning for ad hoc manifests that have
    // not rebuilt reverse_deps.
    if let Some(ref dep_path) = filters.depends_on {
        let exts = builtin_source_extensions();
        let dep_stem = strip_source_ext(dep_path, exts);
        let targets = dependency_query_targets(manifest, dep_path, exts);
        let reverse_matches = transitive_dependents(manifest, &targets);
        file_set.retain(|(file_path, entry)| {
            reverse_matches.contains(file_path.as_str())
                || (reverse_matches.is_empty()
                    && (entry.dependencies.iter().any(|d| {
                        dep_targets_file(d, dep_path, file_path, manifest, exts)
                            || d.contains(dep_stem)
                    }) || entry
                        .imports
                        .iter()
                        .any(|i| dotted_dep_matches(i, dep_path))))
        });
    }

    // LOC range filter.
    file_set.retain(|(_, entry)| {
        filters.min_loc.is_none_or(|min| entry.loc >= min)
            && filters.max_loc.is_none_or(|max| entry.loc <= max)
    });

    let mut results: Vec<FileSearchResult> = file_set
        .into_iter()
        .map(|(path, entry)| file_entry_to_result(path, entry))
        .collect();
    results.sort_by(|a, b| a.file.cmp(&b.file));
    results
}

fn dependency_query_targets(
    manifest: &Manifest,
    dep_path: &str,
    known_extensions: &HashSet<String>,
) -> HashSet<String> {
    let dep_stem = strip_source_ext(dep_path, known_extensions);
    manifest
        .files
        .keys()
        .filter(|path| {
            let path_stem = strip_source_ext(path, known_extensions);
            path.as_str() == dep_path
                || path_stem == dep_stem
                || path_stem.ends_with(&format!("/{dep_stem}"))
        })
        .cloned()
        .collect()
}

fn transitive_dependents(manifest: &Manifest, targets: &HashSet<String>) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();

    for target in targets {
        if let Some(direct) = manifest.reverse_deps.get(target) {
            queue.extend(direct.iter().cloned());
        }
    }

    while let Some(file) = queue.pop_front() {
        if !seen.insert(file.clone()) {
            continue;
        }
        if let Some(next) = manifest.reverse_deps.get(&file) {
            queue.extend(next.iter().filter(|path| !seen.contains(*path)).cloned());
        }
    }

    seen
}

/// Check whether a dependency string `dep` (from file `source`) resolves to `target` in `manifest`.
///
/// For relative imports (`./` or `../`), delegates to `try_resolve_local_dep` which handles
/// extension-agnostic matching AND index-file fallback (`./module` -> `module/index.ts`).
/// For all other dep types, delegates to `dep_matches` and `python_dep_matches`.
fn dep_targets_file(
    dep: &str,
    target: &str,
    source: &str,
    manifest: &Manifest,
    known_extensions: &HashSet<String>,
) -> bool {
    if dep.starts_with("./") || dep.starts_with("../") {
        if let Some(resolved) = try_resolve_local_dep(dep, source, manifest, known_extensions) {
            strip_source_ext(&resolved, known_extensions)
                == strip_source_ext(target, known_extensions)
        } else {
            false
        }
    } else {
        dep_matches(dep, target, source, known_extensions)
            || python_dep_matches(dep, target, source)
    }
}
