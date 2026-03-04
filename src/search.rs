//! Shared search logic for both CLI and MCP.
//!
//! Extracts the duplicated search algorithms from `cli/search.rs` and
//! `mcp/mod.rs` into reusable functions with typed result structs.

use std::collections::{BTreeMap, HashSet};

use crate::manifest::{ExportLocation, FileEntry, Manifest};
use crate::mcp::dep_matches;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A single export hit from a search.
pub struct ExportHit {
    pub name: String,
    pub file: String,
    pub lines: Option<[usize; 2]>,
}

/// A package import hit with all files that use it.
pub struct ImportHit {
    pub package: String,
    pub files: Vec<String>,
}

/// Result of a bare term search (grouped by type).
pub struct BareSearchResult {
    pub exports: Vec<ExportHit>,
    pub files: Vec<String>,
    pub imports: Vec<ImportHit>,
}

/// Per-file search result for filter-based search.
pub struct FileSearchResult {
    pub file: String,
    pub exports: Vec<ExportHitCompact>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}

/// Export name + optional line range (used within FileSearchResult).
pub struct ExportHitCompact {
    pub name: String,
    pub lines: Option<[usize; 2]>,
}

/// Filters for structured search.
pub struct SearchFilters {
    pub export: Option<String>,
    pub imports: Option<String>,
    pub depends_on: Option<String>,
    pub min_loc: Option<usize>,
    pub max_loc: Option<usize>,
}

// ---------------------------------------------------------------------------
// Search functions
// ---------------------------------------------------------------------------

/// Universal term search: searches exports (exact → fuzzy), file paths, imports.
pub fn bare_search(manifest: &Manifest, term: &str) -> BareSearchResult {
    let term_lower = term.to_lowercase();

    // 1. Exact export match (O(1))
    let mut export_hits: Vec<ExportHit> = Vec::new();
    let mut seen_exports = HashSet::new();

    if let Some(loc) = manifest.export_locations.get(term) {
        export_hits.push(export_hit_from_location(term, loc));
        seen_exports.insert(term.to_string());
    } else if let Some(file_path) = manifest.export_index.get(term) {
        export_hits.push(ExportHit {
            name: term.to_string(),
            file: file_path.clone(),
            lines: None,
        });
        seen_exports.insert(term.to_string());
    }

    // 2. Fuzzy export matches (case-insensitive substring, excluding exact)
    let mut fuzzy: Vec<(&str, &ExportLocation)> = manifest
        .export_locations
        .iter()
        .filter(|(name, _)| !seen_exports.contains(name.as_str()))
        .filter(|(name, _)| name.to_lowercase().contains(&term_lower))
        .map(|(name, loc)| (name.as_str(), loc))
        .collect();
    fuzzy.sort_by_key(|(name, _)| name.to_lowercase());

    for (name, loc) in fuzzy {
        export_hits.push(export_hit_from_location(name, loc));
    }

    // 3. File path matches
    let mut file_matches: Vec<String> = manifest
        .files
        .keys()
        .filter(|path| path.to_lowercase().contains(&term_lower))
        .cloned()
        .collect();
    file_matches.sort();

    // 4. Import matches
    let mut import_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (file_path, entry) in &manifest.files {
        for imp in &entry.imports {
            if imp.to_lowercase().contains(&term_lower) {
                import_map
                    .entry(imp.clone())
                    .or_default()
                    .push(file_path.clone());
            }
        }
    }
    for files in import_map.values_mut() {
        files.sort();
    }

    let import_hits: Vec<ImportHit> = import_map
        .into_iter()
        .map(|(package, files)| ImportHit { package, files })
        .collect();

    BareSearchResult {
        exports: export_hits,
        files: file_matches,
        imports: import_hits,
    }
}

/// Structured filter search: export, imports, depends_on, LOC range.
pub fn filter_search(manifest: &Manifest, filters: &SearchFilters) -> Vec<FileSearchResult> {
    let has_export = filters.export.is_some();
    let has_imports = filters.imports.is_some();
    let has_depends_on = filters.depends_on.is_some();

    let mut file_set: Vec<(&String, &FileEntry)> = Vec::new();

    // Search by export — exact first, then fuzzy fallback
    if let Some(ref export) = filters.export {
        if let Some(file_path) = manifest.export_index.get(export.as_str()) {
            if let Some(entry) = manifest.files.get(file_path) {
                file_set.push((file_path, entry));
            }
        } else {
            let export_lower = export.to_lowercase();
            for (name, file_path) in &manifest.export_index {
                if name.to_lowercase().contains(&export_lower) {
                    if let Some(entry) = manifest.files.get(file_path) {
                        if !file_set.iter().any(|(f, _)| *f == file_path) {
                            file_set.push((file_path, entry));
                        }
                    }
                }
            }
        }
    }

    // Search by imports
    if let Some(ref import_name) = filters.imports {
        for (file_path, entry) in &manifest.files {
            if entry
                .imports
                .iter()
                .any(|i| i.contains(import_name.as_str()))
                && !file_set.iter().any(|(f, _)| *f == file_path)
            {
                file_set.push((file_path, entry));
            }
        }
    }

    // Search by depends_on
    if let Some(ref dep_path) = filters.depends_on {
        for (file_path, entry) in &manifest.files {
            if entry
                .dependencies
                .iter()
                .any(|d| d.contains(dep_path.as_str()))
                && !file_set.iter().any(|(f, _)| *f == file_path)
            {
                file_set.push((file_path, entry));
            }
        }
    }

    // LOC filtering
    if filters.min_loc.is_some() || filters.max_loc.is_some() {
        if file_set.is_empty() && !has_export && !has_imports && !has_depends_on {
            for (file_path, entry) in &manifest.files {
                file_set.push((file_path, entry));
            }
        }
        file_set.retain(|(_, entry)| {
            let passes_min = filters.min_loc.is_none_or(|min| entry.loc >= min);
            let passes_max = filters.max_loc.is_none_or(|max| entry.loc <= max);
            passes_min && passes_max
        });
    }

    // If no filters at all, return everything
    if !has_export
        && !has_imports
        && !has_depends_on
        && filters.min_loc.is_none()
        && filters.max_loc.is_none()
    {
        for (file_path, entry) in &manifest.files {
            file_set.push((file_path, entry));
        }
    }

    let mut results: Vec<FileSearchResult> = file_set
        .into_iter()
        .map(|(path, entry)| file_entry_to_result(path, entry))
        .collect();
    results.sort_by(|a, b| a.file.cmp(&b.file));
    results
}

/// Find exports matching a name — exact O(1) first, then case-insensitive substring.
pub fn find_export_matches(manifest: &Manifest, name: &str) -> Vec<ExportHit> {
    let mut hits: Vec<ExportHit> = Vec::new();
    let mut seen = HashSet::new();

    // Exact match (O(1))
    if let Some(loc) = manifest.export_locations.get(name) {
        hits.push(export_hit_from_location(name, loc));
        seen.insert(name.to_string());
    }

    // Fuzzy: case-insensitive substring
    let name_lower = name.to_lowercase();
    let mut fuzzy: Vec<(&str, &ExportLocation)> = manifest
        .export_locations
        .iter()
        .filter(|(export_name, _)| {
            !seen.contains(export_name.as_str()) && export_name.to_lowercase().contains(&name_lower)
        })
        .map(|(export_name, loc)| (export_name.as_str(), loc))
        .collect();
    fuzzy.sort_by(|(a, _), (b, _)| a.to_lowercase().cmp(&b.to_lowercase()));

    for (n, loc) in fuzzy {
        hits.push(export_hit_from_location(n, loc));
    }

    hits
}

/// Compute upstream and downstream dependencies for a file.
pub fn dependency_graph<'a>(
    manifest: &'a Manifest,
    file: &str,
    entry: &'a FileEntry,
) -> (Vec<&'a str>, Vec<&'a String>) {
    let upstream: Vec<&str> = entry.dependencies.iter().map(|s| s.as_str()).collect();

    let mut downstream: Vec<&String> = manifest
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != file)
        .filter(|(path, e)| e.dependencies.iter().any(|d| dep_matches(d, file, path)))
        .map(|(path, _)| path)
        .collect();
    downstream.sort();

    (upstream, downstream)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn export_hit_from_location(name: &str, loc: &ExportLocation) -> ExportHit {
    ExportHit {
        name: name.to_string(),
        file: loc.file.clone(),
        lines: loc.lines.as_ref().map(|l| [l.start, l.end]),
    }
}

fn file_entry_to_result(path: &str, entry: &FileEntry) -> FileSearchResult {
    let exports: Vec<ExportHitCompact> = entry
        .exports
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let lines = entry
                .export_lines
                .as_ref()
                .and_then(|el| el.get(i))
                .filter(|l| l.start > 0)
                .map(|l| [l.start, l.end]);
            ExportHitCompact {
                name: name.clone(),
                lines,
            }
        })
        .collect();

    FileSearchResult {
        file: path.to_string(),
        exports,
        imports: entry.imports.clone(),
        dependencies: entry.dependencies.clone(),
        loc: entry.loc,
    }
}
