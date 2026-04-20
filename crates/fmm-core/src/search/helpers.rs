use std::collections::HashSet;
use std::path::Path;

use crate::manifest::{ExportLocation, FileEntry, Manifest};
use crate::resolver::RustImportResolver;

use super::{ExportHit, ExportHitCompact, FileSearchResult};

pub(super) fn export_hit_from_location(name: &str, loc: &ExportLocation) -> ExportHit {
    ExportHit {
        name: name.to_string(),
        file: loc.file.clone(),
        lines: loc.lines.as_ref().map(|l| [l.start, l.end]),
    }
}

pub(super) fn file_entry_to_result(path: &str, entry: &FileEntry) -> FileSearchResult {
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

pub(super) fn direct_upstream_from_reverse_deps(manifest: &Manifest, file: &str) -> Vec<String> {
    let mut upstream: Vec<String> = manifest
        .reverse_deps
        .iter()
        .filter(|(target, sources)| {
            target.as_str() != file && sources.iter().any(|source| source == file)
        })
        .map(|(target, _)| target.clone())
        .collect();
    upstream.sort();
    upstream.dedup();
    upstream
}

pub(super) fn reverse_deps_resolve_specifier(
    workspace_specifier_names: &[String],
    direct_upstream: &[String],
    specifier: &str,
) -> bool {
    !direct_upstream.is_empty()
        && workspace_specifier_names.iter().any(|name| {
            workspace_package_matches_specifier(name, specifier)
                && direct_upstream
                    .iter()
                    .any(|target| target_matches_workspace_specifier(target, name, specifier))
        })
}

pub(super) fn workspace_specifier_names_for_source(
    manifest: &Manifest,
    rust_resolver: Option<&RustImportResolver>,
    source_file: &str,
) -> Vec<String> {
    let mut names: Vec<String> = manifest.workspace_packages.keys().cloned().collect();
    if source_file.ends_with(".rs")
        && let Some(resolver) = rust_resolver
    {
        names.extend(resolver.workspace_dependency_names_for_importer(Path::new(source_file)));
    }
    names.sort();
    names.dedup();
    names
}

pub(super) fn rust_workspace_resolver(
    manifest: &Manifest,
    source_file: &str,
) -> Option<RustImportResolver> {
    (source_file.ends_with(".rs") && !manifest.workspace_packages.is_empty())
        .then(|| RustImportResolver::new(&manifest.workspace_packages))
}

fn workspace_package_matches_specifier(package: &str, specifier: &str) -> bool {
    specifier == package
        || specifier.strip_prefix(package).is_some_and(|rest| {
            rest.starts_with('/') || rest.starts_with('.') || rest.starts_with("::")
        })
}

fn target_matches_workspace_specifier(target: &str, package: &str, specifier: &str) -> bool {
    let Some(rest) = specifier.strip_prefix(package) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }

    let Some(path) = rest
        .strip_prefix('/')
        .or_else(|| rest.strip_prefix('.'))
        .or_else(|| rest.strip_prefix("::"))
    else {
        return false;
    };
    let first_segment = path
        .split(['/', '.', ':'])
        .find(|segment| !segment.is_empty())
        .unwrap_or("");
    if first_segment.is_empty() {
        return true;
    }

    let normalized_path = path.replace("::", "/").replace('.', "/");
    target.contains(&normalized_path) || target.contains(first_segment)
}

/// Score an export name against a lower-cased search term.
/// Higher score = more relevant. Drives sorting in bare_search fuzzy results.
pub(super) fn export_match_score(name: &str, term_lower: &str) -> u32 {
    let name_lower = name.to_lowercase();
    if name_lower == term_lower {
        return 100;
    }
    if name_lower.starts_with(term_lower) {
        return 80;
    }
    if name_lower.ends_with(term_lower) {
        return 60;
    }
    if let Some(pos) = name_lower.find(term_lower) {
        let before_boundary = pos == 0
            || matches!(
                name_lower.as_bytes().get(pos - 1),
                Some(b'_' | b'.' | b'-' | b':')
            );
        let after_pos = pos + term_lower.len();
        let after_boundary = after_pos >= name_lower.len()
            || matches!(
                name_lower.as_bytes().get(after_pos),
                Some(b'_' | b'.' | b'-' | b':')
            );
        if before_boundary || after_boundary {
            return 40;
        }
        return 20;
    }
    0
}

/// Find exports matching a name: exact O(1) first, then case-insensitive substring.
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
    fuzzy.sort_by_key(|(a, _)| a.to_lowercase());

    for (n, loc) in fuzzy {
        hits.push(export_hit_from_location(n, loc));
    }

    hits
}
