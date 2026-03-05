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
    /// Total fuzzy export hits before the limit was applied. None = no limit applied.
    pub total_exports: Option<usize>,
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

/// Default cap for fuzzy export results in bare_search.
pub const DEFAULT_SEARCH_LIMIT: usize = 50;

/// Universal term search: searches exports (exact → scored fuzzy), file paths, imports.
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

    // 2. Fuzzy export matches — scored by relevance, capped at limit
    let mut fuzzy: Vec<(u32, &str, &ExportLocation)> = manifest
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
        total_exports: if capped { Some(total_fuzzy) } else { None },
    }
}

/// Score an export name against a lower-cased search term.
/// Higher score = more relevant. Drives sorting in bare_search fuzzy results.
pub fn export_match_score(name: &str, term_lower: &str) -> u32 {
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
///
/// Upstream is split into `local` (resolved to files in the manifest) and
/// `external` (package names that could not be resolved). Python relative
/// imports (`._run`, `..config`) are resolved to file paths where possible.
pub fn dependency_graph<'a>(
    manifest: &'a Manifest,
    file: &str,
    entry: &'a FileEntry,
) -> (Vec<String>, Vec<String>, Vec<&'a String>) {
    let mut local: Vec<String> = Vec::new();
    let mut external: Vec<String> = Vec::new();

    for dep in &entry.dependencies {
        if let Some(resolved) = try_resolve_local_dep(dep, file, manifest) {
            if !local.contains(&resolved) {
                local.push(resolved);
            }
        } else if !external.contains(dep) {
            external.push(dep.clone());
        }
    }
    local.sort();
    external.sort();

    let mut downstream: Vec<&String> = manifest
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != file)
        .filter(|(path, e)| {
            e.dependencies
                .iter()
                .any(|d| dep_matches(d, file, path) || python_dep_matches(d, file, path))
        })
        .map(|(path, _)| path)
        .collect();
    downstream.sort();

    (local, external, downstream)
}

/// Attempt to resolve a dependency string to a file path present in the manifest.
///
/// Handles both Python-style relative imports (`._run`, `..config`) and
/// JS/TS-style relative paths (`./utils`, `../config`).
fn try_resolve_local_dep(dep: &str, source_file: &str, manifest: &Manifest) -> Option<String> {
    // Python-style relative imports: start with . but NOT ./ or ../
    if dep.starts_with('.') && !dep.starts_with("./") && !dep.starts_with("../") {
        let resolved_stem = resolve_python_relative_path(dep, source_file)?;
        // Try .py extension first, then package __init__, then exact match
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
        return manifest
            .files
            .keys()
            .find(|path| dep_matches(dep, path, source_file))
            .cloned();
    }
    // Domain-qualified paths: Go module paths (github.com/...) and Rust crate:: paths.
    // dep_matches has suffix-matching fallback for these. Plain external packages like
    // "anyhow" or "fmt" (no "/" or "::") are left as external.
    if dep.contains('/') || dep.contains("::") {
        return manifest
            .files
            .keys()
            .find(|path| dep_matches(dep, path, source_file))
            .cloned();
    }
    None
}

/// Resolve a Python relative import string (e.g. `._run`, `..utils`) to a
/// path stem relative to the project root, based on `source_file`'s location.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{FileEntry, Manifest};
    use crate::parser::{ExportEntry, Metadata};

    #[allow(dead_code)]
    fn make_entry(deps: Vec<&str>, loc: usize) -> FileEntry {
        FileEntry {
            exports: vec![],
            export_lines: None,
            imports: vec![],
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            loc,
        }
    }

    fn manifest_with(files: Vec<(&str, Vec<&str>)>) -> Manifest {
        let mut m = Manifest::new();
        for (path, deps) in files {
            m.add_file(
                path,
                Metadata {
                    exports: vec![ExportEntry::new(path.to_string(), 1, 1)],
                    imports: vec![],
                    dependencies: deps.iter().map(|s| s.to_string()).collect(),
                    loc: 10,
                },
            );
        }
        m
    }

    #[test]
    fn python_dep_matches_single_dot() {
        // `from ._run import X` in `agno/agent/agent.py` should match `agno/agent/_run.py`
        assert!(python_dep_matches(
            "._run",
            "agno/agent/_run.py",
            "agno/agent/agent.py"
        ));
        assert!(!python_dep_matches(
            "._run",
            "agno/agent/other.py",
            "agno/agent/agent.py"
        ));
    }

    #[test]
    fn python_dep_matches_double_dot() {
        // `from ..config import X` in `agno/agent/agent.py` should match `agno/config.py`
        assert!(python_dep_matches(
            "..config",
            "agno/config.py",
            "agno/agent/agent.py"
        ));
        assert!(!python_dep_matches(
            "..config",
            "agno/agent/config.py",
            "agno/agent/agent.py"
        ));
    }

    #[test]
    fn python_dep_matches_dot_only_returns_false() {
        // `from . import X` — can't resolve to a specific file
        assert!(!python_dep_matches(
            ".",
            "agno/agent/_run.py",
            "agno/agent/agent.py"
        ));
    }

    #[test]
    fn python_dep_does_not_match_js_style() {
        // Should not match JS/TS style paths — those are handled by dep_matches
        assert!(!python_dep_matches(
            "./utils",
            "src/utils.ts",
            "src/index.ts"
        ));
    }

    #[test]
    fn dependency_graph_resolves_python_deps() {
        let manifest = manifest_with(vec![
            ("agno/agent/_run.py", vec![]),
            ("agno/agent/models.py", vec![]),
            (
                "agno/agent/agent.py",
                vec!["._run", ".models", "pydantic", "typing"],
            ),
        ]);
        // Reload from add_file — dependencies are stored as-is
        let entry = manifest.files["agno/agent/agent.py"].clone();

        let (local, external, downstream) =
            dependency_graph(&manifest, "agno/agent/agent.py", &entry);

        assert!(
            local.contains(&"agno/agent/_run.py".to_string()),
            "should resolve ._run, got: {:?}",
            local
        );
        assert!(
            local.contains(&"agno/agent/models.py".to_string()),
            "should resolve .models, got: {:?}",
            local
        );
        assert!(
            external.contains(&"pydantic".to_string()),
            "pydantic should stay external, got: {:?}",
            external
        );
        assert!(
            external.contains(&"typing".to_string()),
            "typing should stay external, got: {:?}",
            external
        );
        assert!(downstream.is_empty(), "no downstream expected");
    }

    #[test]
    fn dependency_graph_downstream_detects_python_dependents() {
        let manifest = manifest_with(vec![
            ("agno/agent/_run.py", vec![]),
            (
                "agno/agent/agent.py",
                vec!["._run"], // agent.py depends on _run.py via relative import
            ),
        ]);
        let entry = manifest.files["agno/agent/_run.py"].clone();

        let (_, _, downstream) = dependency_graph(&manifest, "agno/agent/_run.py", &entry);

        assert!(
            downstream.contains(&&"agno/agent/agent.py".to_string()),
            "agent.py should appear as downstream of _run.py, got: {:?}",
            downstream
        );
    }

    #[test]
    fn export_match_score_exact() {
        assert_eq!(export_match_score("Agent", "agent"), 100);
    }

    #[test]
    fn export_match_score_prefix() {
        assert_eq!(export_match_score("AgentConfig", "agent"), 80);
    }

    #[test]
    fn export_match_score_suffix() {
        assert_eq!(export_match_score("MockAgent", "agent"), 60);
    }

    #[test]
    fn export_match_score_word_boundary() {
        assert_eq!(export_match_score("run_agent_loop", "agent"), 40);
    }

    #[test]
    fn export_match_score_substring() {
        // "ck" in "buckets" is a mid-word substring (no boundary around it)
        assert_eq!(export_match_score("buckets_handler", "ck"), 20);
    }

    #[test]
    fn bare_search_scores_prefix_before_suffix() {
        let manifest = manifest_with(vec![
            ("src/mock_agent.py", vec![]),
            ("src/agent_config.py", vec![]),
        ]);
        // Add exports manually via add_file with ExportEntry
        let result = bare_search(&manifest, "agent", None);
        // AgentConfig (prefix) should come before MockAgent (suffix)
        let names: Vec<&str> = result.exports.iter().map(|h| h.name.as_str()).collect();
        if let (Some(ag_pos), Some(mock_pos)) = (
            names
                .iter()
                .position(|&n| n.contains("AgentConfig") || n == "agent_config.py"),
            names
                .iter()
                .position(|&n| n.contains("MockAgent") || n == "mock_agent.py"),
        ) {
            // prefix match should rank higher than suffix match
            assert!(
                ag_pos <= mock_pos,
                "Expected prefix match before suffix match, got: {:?}",
                names
            );
        }
    }

    #[test]
    fn bare_search_limit_caps_results() {
        let mut manifest = Manifest::new();
        use crate::parser::{ExportEntry, Metadata};
        // Add 10 exports all containing "foo"
        for i in 0..10 {
            manifest.add_file(
                &format!("src/mod{}.py", i),
                Metadata {
                    exports: vec![ExportEntry::new(format!("FooHandler{}", i), 1, 5)],
                    imports: vec![],
                    dependencies: vec![],
                    loc: 10,
                },
            );
        }
        let result = bare_search(&manifest, "foo", Some(3));
        // Should cap fuzzy results at 3
        assert!(
            result.exports.len() <= 3,
            "expected at most 3 results, got {}",
            result.exports.len()
        );
        assert!(
            result.total_exports.is_some(),
            "should report total when capped"
        );
        assert_eq!(result.total_exports.unwrap(), 10);
    }
}
