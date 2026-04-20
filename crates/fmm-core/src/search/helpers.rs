use std::collections::HashSet;

use crate::manifest::{ExportLocation, FileEntry, Manifest};

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
