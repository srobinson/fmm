use std::collections::{BTreeMap, HashSet};

use crate::manifest::Manifest;

use super::helpers::{export_hit_from_location, export_match_score};
use super::{BareSearchResult, DEFAULT_SEARCH_LIMIT, ExportHit, ImportHit, NamedImportHit};

/// Universal term search: searches exports (exact -> scored fuzzy), file paths, imports.
/// `limit` caps fuzzy export results (default: 50). Exact match always included.
pub fn bare_search(manifest: &Manifest, term: &str, limit: Option<usize>) -> BareSearchResult {
    let term_lower = term.to_lowercase();
    let cap = limit.unwrap_or(DEFAULT_SEARCH_LIMIT);

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

    // 2. Exact match in method_index by full dotted name (e.g. "createTypeChecker.getIndexType").
    let method_exact = manifest
        .method_index
        .get(term)
        .map(|loc| (term.to_string(), loc.clone()));
    if let Some((name, loc)) = method_exact
        && !seen_exports.contains(&name)
    {
        export_hits.push(export_hit_from_location(&name, &loc));
        seen_exports.insert(name);
    }

    // 2b. Fuzzy method_index search: dotted names that contain the term.
    // This is how "silentNeverType" finds "createTypeChecker.silentNeverType".
    let mut method_fuzzy: Vec<(u32, String, &crate::manifest::ExportLocation)> = manifest
        .method_index
        .iter()
        .filter(|(name, _)| !seen_exports.contains(name.as_str()))
        .filter(|(name, _)| {
            let name_lower = name.to_lowercase();
            name_lower.contains(&term_lower)
        })
        .map(|(name, loc)| {
            // Score against the last component (after the dot) for better relevance
            let short = name.rfind('.').map(|p| &name[p + 1..]).unwrap_or(name);
            (export_match_score(short, &term_lower), name.clone(), loc)
        })
        .collect();
    method_fuzzy.sort_by(|(sa, na, _), (sb, nb, _)| sb.cmp(sa).then(na.cmp(nb)));
    for (_, name, loc) in method_fuzzy.into_iter().take(cap) {
        if seen_exports.insert(name.clone()) {
            export_hits.push(export_hit_from_location(&name, loc));
        }
    }

    // 3. Fuzzy export matches: scored by relevance, capped at limit
    let mut fuzzy: Vec<(u32, &str, &crate::manifest::ExportLocation)> = manifest
        .export_locations
        .iter()
        .filter(|(name, _)| !seen_exports.contains(name.as_str()))
        .filter(|(name, _)| name.to_lowercase().contains(&term_lower))
        .map(|(name, loc)| (export_match_score(name, &term_lower), name.as_str(), loc))
        .collect();
    // Score descending, then alphabetically for ties
    fuzzy.sort_by(|(sa, na, _), (sb, nb, _)| sb.cmp(sa).then(na.cmp(nb)));

    let total_fuzzy = fuzzy.len();
    let capped = total_fuzzy > cap;

    for (_, name, loc) in fuzzy.into_iter().take(cap) {
        export_hits.push(export_hit_from_location(name, loc));
        seen_exports.insert(name.to_string());
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

    // 5. Named-import call-site matches: files that import `term` by name from any source.
    // Groups by (symbol, source_package) so the caller sees which package each symbol came from.
    // Key: (symbol, source) -> files
    let mut named_import_map: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (file_path, entry) in &manifest.files {
        for (source, symbols) in &entry.named_imports {
            for symbol in symbols {
                if symbol.to_lowercase().contains(&term_lower) {
                    named_import_map
                        .entry((symbol.clone(), source.clone()))
                        .or_default()
                        .push(file_path.clone());
                }
            }
        }
    }
    for files in named_import_map.values_mut() {
        files.sort();
    }

    let named_import_hits: Vec<NamedImportHit> = named_import_map
        .into_iter()
        .map(|((symbol, source), files)| NamedImportHit {
            symbol,
            source,
            files,
        })
        .collect();

    BareSearchResult {
        exports: export_hits,
        files: file_matches,
        imports: import_hits,
        named_import_hits,
        total_exports: if capped { Some(total_fuzzy) } else { None },
    }
}
