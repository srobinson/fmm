//! Shared search logic for both CLI and MCP.
//!
//! Extracts the duplicated search algorithms from `cli/search.rs` and
//! `mcp/mod.rs` into reusable functions with typed result structs.

use std::collections::{BTreeMap, HashSet};

use crate::manifest::{
    dep_matches, dotted_dep_matches, python_dep_matches, strip_source_ext, try_resolve_local_dep,
    ExportLocation, FileEntry, Manifest,
};

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
///
/// All active filters are combined with AND semantics — results must satisfy every
/// specified constraint. Previously filters used OR (union) semantics which caused
/// silent result pollution when multiple filters were combined (ALP-823).
pub fn filter_search(manifest: &Manifest, filters: &SearchFilters) -> Vec<FileSearchResult> {
    let has_export = filters.export.is_some();
    let has_imports = filters.imports.is_some();
    let has_depends_on = filters.depends_on.is_some();
    let has_loc = filters.min_loc.is_some() || filters.max_loc.is_some();

    // No filters at all → return everything.
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
    let mut file_set: Vec<(&String, &FileEntry)> = manifest.files.iter().collect();

    // Export filter — exact O(1) first, then case-insensitive substring fallback.
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

    // Imports filter — file must import the given package/module name.
    if let Some(ref import_name) = filters.imports {
        file_set.retain(|(_, entry)| {
            entry
                .imports
                .iter()
                .any(|i| i.contains(import_name.as_str()))
        });
    }

    // depends_on filter — same resolution logic as dependency_graph downstream computation.
    // Relative paths (./ ../), extension variants (.ts/.js), Python dot-imports and
    // Go/Rust module paths all resolve via dep_targets_file; substring fallback handles
    // bare fragment queries (e.g. "config" matches "../config").
    if let Some(ref dep_path) = filters.depends_on {
        let dep_stem = strip_source_ext(dep_path);
        file_set.retain(|(file_path, entry)| {
            entry
                .dependencies
                .iter()
                .any(|d| dep_targets_file(d, dep_path, file_path, manifest) || d.contains(dep_stem))
                || entry
                    .imports
                    .iter()
                    .any(|i| dotted_dep_matches(i, dep_path))
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
    // entry.imports are classified as external by the parser. Package paths containing '/'
    // (npm scoped packages like `@nestjs/common/services/logger.service`, deep module paths)
    // are always external — passing them through try_resolve_local_dep causes ghost local_deps
    // via suffix matching (e.g. `logger.service` matches `transient-logger.service.ts`).
    // Only dotted imports without '/' are tried as potential local files (Python absolute
    // imports like `agno.models.message`).
    for imp in &entry.imports {
        if !imp.contains('/') {
            if let Some(resolved) = try_resolve_local_dep(imp, file, manifest) {
                if !local.contains(&resolved) {
                    local.push(resolved);
                }
                continue;
            }
        }
        if !external.contains(imp) {
            external.push(imp.clone());
        }
    }
    local.sort();
    external.sort();

    // O(1) lookup using the pre-built reverse dependency index (built at manifest load time).
    // The index maps each file to the sorted list of files that directly import it.
    let mut downstream: Vec<&String> = manifest
        .reverse_deps
        .get(file)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    downstream.sort();

    (local, external, downstream)
}

/// Transitive dependency traversal with BFS and cycle detection.
///
/// Returns `(upstream, external, downstream)`:
/// - `upstream`: local dep file paths discovered by traversal, each annotated with
///   the hop depth at which it was first reached
/// - `external`: unresolvable dep strings (packages, etc.), deduplicated and sorted
/// - `downstream`: files that transitively depend on `file`, depth-annotated
///
/// `depth=1` gives the same results as `dependency_graph()` but with depth annotations.
/// `depth=N` traverses N hops. `depth=-1` computes the full transitive closure.
/// Cycle detection via `HashSet<String>` — already-visited files are never re-queued.
#[allow(clippy::type_complexity)]
pub fn dependency_graph_transitive(
    manifest: &Manifest,
    file: &str,
    entry: &FileEntry,
    depth: i32,
) -> (Vec<(String, i32)>, Vec<String>, Vec<(String, i32)>) {
    use std::collections::{BTreeSet, HashSet, VecDeque};

    // -------------------------------------------------------------------------
    // Upstream BFS
    // -------------------------------------------------------------------------
    let mut upstream: Vec<(String, i32)> = Vec::new();
    let mut visited_up: HashSet<String> = HashSet::new();
    visited_up.insert(file.to_string());
    let mut external_set: BTreeSet<String> = BTreeSet::new();

    let mut queue_up: VecDeque<(String, i32)> = VecDeque::new();
    for dep in &entry.dependencies {
        if let Some(resolved) = try_resolve_local_dep(dep, file, manifest) {
            if !visited_up.contains(&resolved) {
                queue_up.push_back((resolved, 1));
            }
        } else {
            external_set.insert(dep.clone());
        }
    }
    for imp in &entry.imports {
        if !imp.contains('/') {
            if let Some(resolved) = try_resolve_local_dep(imp, file, manifest) {
                if !visited_up.contains(&resolved) {
                    queue_up.push_back((resolved, 1));
                }
                continue;
            }
        }
        external_set.insert(imp.clone());
    }

    while let Some((current, d)) = queue_up.pop_front() {
        if visited_up.contains(&current) {
            continue;
        }
        visited_up.insert(current.clone());
        upstream.push((current.clone(), d));

        if depth == -1 || d < depth {
            if let Some(e) = manifest.files.get(&current) {
                for dep in &e.dependencies {
                    if let Some(resolved) = try_resolve_local_dep(dep, &current, manifest) {
                        if !visited_up.contains(&resolved) {
                            queue_up.push_back((resolved, d + 1));
                        }
                    } else {
                        external_set.insert(dep.clone());
                    }
                }
                for imp in &e.imports {
                    if !imp.contains('/') {
                        if let Some(resolved) = try_resolve_local_dep(imp, &current, manifest) {
                            if !visited_up.contains(&resolved) {
                                queue_up.push_back((resolved, d + 1));
                            }
                            continue;
                        }
                    }
                    external_set.insert(imp.clone());
                }
            }
        }
    }

    upstream.sort_by(|a, b| a.0.cmp(&b.0));
    let external: Vec<String> = external_set.into_iter().collect();

    // -------------------------------------------------------------------------
    // Downstream BFS
    // -------------------------------------------------------------------------
    let mut downstream: Vec<(String, i32)> = Vec::new();
    let mut visited_down: HashSet<String> = HashSet::new();
    visited_down.insert(file.to_string());

    let mut queue_down: VecDeque<(String, i32)> = VecDeque::new();

    // Seed with files that directly depend on the start file (O(1) reverse index lookup)
    if let Some(direct) = manifest.reverse_deps.get(file) {
        for path in direct {
            if !visited_down.contains(path.as_str()) {
                queue_down.push_back((path.clone(), 1));
            }
        }
    }

    while let Some((current, d)) = queue_down.pop_front() {
        if visited_down.contains(&current) {
            continue;
        }
        visited_down.insert(current.clone());
        downstream.push((current.clone(), d));

        if depth == -1 || d < depth {
            // Expand next hop using reverse index (O(1) per hop instead of O(N))
            if let Some(dependents) = manifest.reverse_deps.get(&current) {
                for path in dependents {
                    if !visited_down.contains(path.as_str()) {
                        queue_down.push_back((path.clone(), d + 1));
                    }
                }
            }
        }
    }

    downstream.sort_by(|a, b| a.0.cmp(&b.0));

    (upstream, external, downstream)
}

/// Check whether a dependency string `dep` (from file `source`) resolves to `target` in `manifest`.
///
/// For relative imports (`./` or `../`), delegates to `try_resolve_local_dep` which handles
/// extension-agnostic matching AND index-file fallback (`./module` → `module/index.ts`).
/// For all other dep types, delegates to `dep_matches` and `python_dep_matches`.
fn dep_targets_file(dep: &str, target: &str, source: &str, manifest: &Manifest) -> bool {
    if dep.starts_with("./") || dep.starts_with("../") {
        if let Some(resolved) = try_resolve_local_dep(dep, source, manifest) {
            strip_source_ext(&resolved) == strip_source_ext(target)
        } else {
            false
        }
    } else {
        dep_matches(dep, target, source) || python_dep_matches(dep, target, source)
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
            methods: None,
            imports: vec![],
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            loc,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
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
                    ..Default::default()
                },
            );
        }
        m.rebuild_reverse_deps();
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

    fn manifest_with_imports(files: Vec<(&str, Vec<&str>, Vec<&str>)>) -> Manifest {
        let mut m = Manifest::new();
        for (path, deps, imps) in files {
            m.add_file(
                path,
                Metadata {
                    exports: vec![ExportEntry::new(path.to_string(), 1, 1)],
                    imports: imps.iter().map(|s| s.to_string()).collect(),
                    dependencies: deps.iter().map(|s| s.to_string()).collect(),
                    loc: 10,
                    ..Default::default()
                },
            );
        }
        m.rebuild_reverse_deps();
        m
    }

    #[test]
    fn dotted_dep_matches_basic() {
        assert!(dotted_dep_matches(
            "agno.models.message",
            "agno/models/message.py"
        ));
        assert!(dotted_dep_matches(
            "agno.models.message",
            "src/agno/models/message.py"
        ));
    }

    #[test]
    fn dotted_dep_matches_package_init() {
        assert!(dotted_dep_matches("agno.models", "agno/models/__init__.py"));
        assert!(dotted_dep_matches(
            "agno.models",
            "src/agno/models/__init__.py"
        ));
    }

    #[test]
    fn dotted_dep_matches_ignores_relative_and_paths() {
        // Relative imports are NOT dotted_dep
        assert!(!dotted_dep_matches("._run", "agno/agent/_run.py"));
        assert!(!dotted_dep_matches("./utils", "utils.py"));
        assert!(!dotted_dep_matches("os", "os.py")); // no dot
        assert!(!dotted_dep_matches("crate::config", "src/config.rs")); // ::
    }

    #[test]
    fn dependency_graph_resolves_dotted_absolute_imports() {
        let manifest = manifest_with_imports(vec![
            ("agno/models/message.py", vec![], vec![]),
            ("agno/models/response.py", vec![], vec![]),
            (
                "agno/models/interfaces.py",
                vec![],
                vec!["agno.models.message", "agno.models.response", "typing"],
            ),
        ]);
        let entry = manifest.files["agno/models/interfaces.py"].clone();

        let (local, external, downstream) =
            dependency_graph(&manifest, "agno/models/interfaces.py", &entry);

        assert!(
            local.contains(&"agno/models/message.py".to_string()),
            "should resolve agno.models.message, got local: {:?}",
            local
        );
        assert!(
            local.contains(&"agno/models/response.py".to_string()),
            "should resolve agno.models.response, got local: {:?}",
            local
        );
        assert!(
            external.contains(&"typing".to_string()),
            "typing should stay external, got: {:?}",
            external
        );
        assert!(downstream.is_empty(), "no downstream expected");
    }

    #[test]
    fn dependency_graph_dotted_downstream_detection() {
        let manifest = manifest_with_imports(vec![
            ("agno/models/message.py", vec![], vec![]),
            (
                "agno/models/interfaces.py",
                vec![],
                vec!["agno.models.message"],
            ),
        ]);
        let entry = manifest.files["agno/models/message.py"].clone();

        let (_, _, downstream) = dependency_graph(&manifest, "agno/models/message.py", &entry);

        assert!(
            downstream.contains(&&"agno/models/interfaces.py".to_string()),
            "interfaces.py should appear as downstream of message.py, got: {:?}",
            downstream
        );
    }

    #[test]
    fn dependency_graph_dotted_src_layout() {
        // Projects with src/ prefix: `from agno.models.message import X`
        // should resolve to `src/agno/models/message.py`
        let manifest = manifest_with_imports(vec![
            ("src/agno/models/message.py", vec![], vec![]),
            (
                "src/agno/models/interfaces.py",
                vec![],
                vec!["agno.models.message"],
            ),
        ]);
        let entry = manifest.files["src/agno/models/interfaces.py"].clone();

        let (local, _, _) = dependency_graph(&manifest, "src/agno/models/interfaces.py", &entry);

        assert!(
            local.contains(&"src/agno/models/message.py".to_string()),
            "src layout: should resolve agno.models.message → src/agno/models/message.py, got: {:?}",
            local
        );
    }

    #[test]
    fn dependency_graph_no_ghost_from_scoped_package_imports() {
        // Regression: imports like `@nestjs/common/services/logger.service` were suffix-matched
        // against local files (e.g. `logger.service` matched `transient-logger.service.ts`).
        // Package paths containing '/' must never be resolved locally.
        let manifest = manifest_with_imports(vec![
            ("src/logger/transient-logger.service.ts", vec![], vec![]),
            (
                "src/nest-factory.ts",
                vec![],
                vec![
                    "@nestjs/common",
                    "@nestjs/common/services/logger.service",
                    "rxjs",
                ],
            ),
        ]);
        let entry = manifest.files["src/nest-factory.ts"].clone();

        let (local, external, _) = dependency_graph(&manifest, "src/nest-factory.ts", &entry);

        assert!(
            local.is_empty(),
            "ghost entry: scoped package imports must not resolve to local files, got local: {:?}",
            local
        );
        assert!(
            external.contains(&"@nestjs/common".to_string()),
            "external should contain @nestjs/common, got: {:?}",
            external
        );
        assert!(
            external.contains(&"@nestjs/common/services/logger.service".to_string()),
            "external should contain deep package path, got: {:?}",
            external
        );
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
                    ..Default::default()
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

    // -------------------------------------------------------------------------
    // dependency_graph_transitive — BFS + cycle detection (ALP-787)
    // -------------------------------------------------------------------------

    /// Build linear chain: app/a.py -> app/b.py -> app/c.py -> app/d.py
    fn chain_manifest() -> Manifest {
        manifest_with(vec![
            ("app/a.py", vec![".b"]),
            ("app/b.py", vec![".c"]),
            ("app/c.py", vec![".d"]),
            ("app/d.py", vec![]),
        ])
    }

    #[test]
    fn transitive_upstream_depth1_matches_single_hop() {
        let m = chain_manifest();
        let entry = m.files["app/a.py"].clone();
        let (upstream, _ext, downstream) = dependency_graph_transitive(&m, "app/a.py", &entry, 1);

        let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(up_files, ["app/b.py"], "depth=1 upstream: direct dep only");
        assert!(
            upstream.iter().all(|(_, d)| *d == 1),
            "all depth=1 entries marked with d=1"
        );
        assert!(
            downstream.is_empty(),
            "nothing depends on app/a.py in the chain"
        );
    }

    #[test]
    fn transitive_upstream_depth2_follows_two_hops() {
        let m = chain_manifest();
        let entry = m.files["app/a.py"].clone();
        let (upstream, _ext, _) = dependency_graph_transitive(&m, "app/a.py", &entry, 2);

        let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
        assert!(
            up_files.contains(&"app/b.py"),
            "app/b.py at depth 1; got: {:?}",
            up_files
        );
        assert!(
            up_files.contains(&"app/c.py"),
            "app/c.py at depth 2; got: {:?}",
            up_files
        );
        assert!(
            !up_files.contains(&"app/d.py"),
            "app/d.py should be beyond depth=2; got: {:?}",
            up_files
        );
        let b_depth = upstream.iter().find(|(f, _)| f == "app/b.py").unwrap().1;
        let c_depth = upstream.iter().find(|(f, _)| f == "app/c.py").unwrap().1;
        assert_eq!(b_depth, 1);
        assert_eq!(c_depth, 2);
    }

    #[test]
    fn transitive_upstream_full_closure() {
        let m = chain_manifest();
        let entry = m.files["app/a.py"].clone();
        let (upstream, _ext, _) = dependency_graph_transitive(&m, "app/a.py", &entry, -1);

        let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
        assert!(up_files.contains(&"app/b.py"), "b in closure");
        assert!(up_files.contains(&"app/c.py"), "c in closure");
        assert!(up_files.contains(&"app/d.py"), "d in closure");
    }

    #[test]
    fn transitive_downstream_multi_hop() {
        let m = chain_manifest();
        let entry = m.files["app/d.py"].clone();
        // d is depended on by c (depth 1), b (depth 2), a (depth 3)
        let (_up, _ext, downstream) = dependency_graph_transitive(&m, "app/d.py", &entry, -1);

        let down_files: Vec<&str> = downstream.iter().map(|(f, _)| f.as_str()).collect();
        assert!(
            down_files.contains(&"app/c.py"),
            "c depends on d at depth 1"
        );
        assert!(
            down_files.contains(&"app/b.py"),
            "b depends on c at depth 2"
        );
        assert!(
            down_files.contains(&"app/a.py"),
            "a depends on b at depth 3"
        );

        let c_depth = downstream.iter().find(|(f, _)| f == "app/c.py").unwrap().1;
        let b_depth = downstream.iter().find(|(f, _)| f == "app/b.py").unwrap().1;
        let a_depth = downstream.iter().find(|(f, _)| f == "app/a.py").unwrap().1;
        assert_eq!(c_depth, 1);
        assert_eq!(b_depth, 2);
        assert_eq!(a_depth, 3);
    }

    #[test]
    fn transitive_cycle_does_not_loop() {
        // Circular: x depends on y, y depends on x
        let m = manifest_with(vec![("app/x.py", vec![".y"]), ("app/y.py", vec![".x"])]);
        let entry = m.files["app/x.py"].clone();
        // Should terminate without infinite loop
        let (upstream, _ext, downstream) = dependency_graph_transitive(&m, "app/x.py", &entry, -1);

        // x's upstream: y (depth 1). x itself is not revisited.
        let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
        assert!(up_files.contains(&"app/y.py"), "y is upstream of x");
        assert!(
            !up_files.contains(&"app/x.py"),
            "x must not appear in its own upstream"
        );

        // x's downstream: y depends on x, so y at depth 1
        let down_files: Vec<&str> = downstream.iter().map(|(f, _)| f.as_str()).collect();
        assert!(
            down_files.contains(&"app/y.py"),
            "y depends on x so appears downstream; got: {:?}",
            down_files
        );
        assert!(
            !down_files.contains(&"app/x.py"),
            "x must not appear in its own downstream"
        );
    }

    // -------------------------------------------------------------------------
    // JS/TS directory-style and extensionless import resolution (ALP-801)
    // -------------------------------------------------------------------------

    #[test]
    fn js_index_ts_resolves_for_directory_import() {
        let m = manifest_with(vec![
            ("src/auth/module/index.ts", vec![]),
            ("src/auth/session.ts", vec!["./module"]),
        ]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, external, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.contains(&"src/auth/module/index.ts".to_string()),
            "./module should resolve to module/index.ts, got local: {:?}",
            local
        );
        assert!(
            external.is_empty(),
            "no external expected, got: {:?}",
            external
        );
    }

    #[test]
    fn js_index_tsx_resolves_for_directory_import() {
        let m = manifest_with(vec![
            ("src/components/Button/index.tsx", vec![]),
            ("src/App.tsx", vec!["./components/Button"]),
        ]);
        let entry = m.files["src/App.tsx"].clone();
        let (local, external, _) = dependency_graph(&m, "src/App.tsx", &entry);
        assert!(
            local.contains(&"src/components/Button/index.tsx".to_string()),
            "./components/Button should resolve to index.tsx, got: {:?}",
            local
        );
        assert!(
            external.is_empty(),
            "no external expected, got: {:?}",
            external
        );
    }

    #[test]
    fn js_index_js_resolves_for_directory_import() {
        let m = manifest_with(vec![
            ("src/utils/index.js", vec![]),
            ("src/app.js", vec!["./utils"]),
        ]);
        let entry = m.files["src/app.js"].clone();
        let (local, _, _) = dependency_graph(&m, "src/app.js", &entry);
        assert!(
            local.contains(&"src/utils/index.js".to_string()),
            "./utils should resolve to utils/index.js, got: {:?}",
            local
        );
    }

    #[test]
    fn js_index_jsx_resolves_for_directory_import() {
        let m = manifest_with(vec![
            ("src/components/Form/index.jsx", vec![]),
            ("src/Page.jsx", vec!["./components/Form"]),
        ]);
        let entry = m.files["src/Page.jsx"].clone();
        let (local, _, _) = dependency_graph(&m, "src/Page.jsx", &entry);
        assert!(
            local.contains(&"src/components/Form/index.jsx".to_string()),
            "./components/Form should resolve to index.jsx, got: {:?}",
            local
        );
    }

    #[test]
    fn js_direct_file_takes_priority_over_index() {
        // When both `module.ts` and `module/index.ts` exist, direct file wins
        let m = manifest_with(vec![
            ("src/auth/module.ts", vec![]),
            ("src/auth/module/index.ts", vec![]),
            ("src/auth/session.ts", vec!["./module"]),
        ]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, _, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.contains(&"src/auth/module.ts".to_string()),
            "direct file should take priority over index.ts, got: {:?}",
            local
        );
        // Exactly one match — no duplicate
        let count = local.iter().filter(|f| f.contains("module")).count();
        assert_eq!(
            count, 1,
            "should resolve to exactly one file, got: {:?}",
            local
        );
    }

    #[test]
    fn js_extensionless_import_resolves_ts() {
        // `./instance-wrapper` → `instance-wrapper.ts` via stem comparison
        let m = manifest_with(vec![
            ("src/logger/transient-logger.service.ts", vec![]),
            ("src/auth/instance-wrapper.ts", vec![]),
            ("src/auth/session.ts", vec!["./instance-wrapper"]),
        ]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, _, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.contains(&"src/auth/instance-wrapper.ts".to_string()),
            "./instance-wrapper should resolve to instance-wrapper.ts, got: {:?}",
            local
        );
        assert!(
            !local.contains(&"src/logger/transient-logger.service.ts".to_string()),
            "should not ghost-match transient-logger.service.ts"
        );
    }

    #[test]
    fn js_extensionless_import_resolves_tsx() {
        let m = manifest_with(vec![
            ("src/components/Header.tsx", vec![]),
            ("src/App.tsx", vec!["./components/Header"]),
        ]);
        let entry = m.files["src/App.tsx"].clone();
        let (local, _, _) = dependency_graph(&m, "src/App.tsx", &entry);
        assert!(
            local.contains(&"src/components/Header.tsx".to_string()),
            "./components/Header should resolve to Header.tsx, got: {:?}",
            local
        );
    }

    #[test]
    fn js_extensionless_import_resolves_js() {
        let m = manifest_with(vec![
            ("lib/helpers.js", vec![]),
            ("lib/main.js", vec!["./helpers"]),
        ]);
        let entry = m.files["lib/main.js"].clone();
        let (local, _, _) = dependency_graph(&m, "lib/main.js", &entry);
        assert!(
            local.contains(&"lib/helpers.js".to_string()),
            "./helpers should resolve to helpers.js, got: {:?}",
            local
        );
    }

    #[test]
    fn js_extensionless_import_resolves_jsx() {
        let m = manifest_with(vec![
            ("src/Button.jsx", vec![]),
            ("src/index.jsx", vec!["./Button"]),
        ]);
        let entry = m.files["src/index.jsx"].clone();
        let (local, _, _) = dependency_graph(&m, "src/index.jsx", &entry);
        assert!(
            local.contains(&"src/Button.jsx".to_string()),
            "./Button should resolve to Button.jsx, got: {:?}",
            local
        );
    }

    #[test]
    fn js_parent_relative_resolves_direct_file() {
        let m = manifest_with(vec![
            ("src/errors/exceptions.ts", vec![]),
            ("src/auth/session.ts", vec!["../errors/exceptions"]),
        ]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, external, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.contains(&"src/errors/exceptions.ts".to_string()),
            "../errors/exceptions should resolve to exceptions.ts, got: {:?}",
            local
        );
        assert!(
            external.is_empty(),
            "no external expected, got: {:?}",
            external
        );
    }

    #[test]
    fn js_parent_relative_resolves_index_file() {
        let m = manifest_with(vec![
            ("src/errors/index.ts", vec![]),
            ("src/auth/session.ts", vec!["../errors"]),
        ]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, external, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.contains(&"src/errors/index.ts".to_string()),
            "../errors should resolve to errors/index.ts, got: {:?}",
            local
        );
        assert!(
            external.is_empty(),
            "no external expected, got: {:?}",
            external
        );
    }

    #[test]
    fn js_deep_nesting_index_resolution() {
        // From `src/app.ts`, `../../shared/utils` goes up past src/ and root,
        // then down to `shared/utils` — i.e. `shared/utils/index.ts`.
        // (excess `..` beyond root are silently clamped to root)
        let m = manifest_with(vec![
            ("shared/utils/index.ts", vec![]),
            ("src/app.ts", vec!["../../shared/utils"]),
        ]);
        let entry = m.files["src/app.ts"].clone();
        let (local, _, _) = dependency_graph(&m, "src/app.ts", &entry);
        assert!(
            local.contains(&"shared/utils/index.ts".to_string()),
            "../../shared/utils (from src/) should resolve to shared/utils/index.ts, got: {:?}",
            local
        );
    }

    #[test]
    fn js_unresolvable_relative_stays_in_external() {
        // A relative path that has no matching file stays external
        let m = manifest_with(vec![("src/auth/session.ts", vec!["./nonexistent-module"])]);
        let entry = m.files["src/auth/session.ts"].clone();
        let (local, external, _) = dependency_graph(&m, "src/auth/session.ts", &entry);
        assert!(
            local.is_empty(),
            "unresolvable relative import must not produce ghost local_dep, got: {:?}",
            local
        );
        assert!(
            external.contains(&"./nonexistent-module".to_string()),
            "unresolvable relative import should appear in external, got: {:?}",
            external
        );
    }

    #[test]
    fn js_index_resolution_does_not_match_wrong_directory() {
        // `./auth` must not match `src/authentication/index.ts`
        let m = manifest_with(vec![
            ("src/authentication/index.ts", vec![]),
            ("src/app.ts", vec!["./auth"]),
        ]);
        let entry = m.files["src/app.ts"].clone();
        let (local, _, _) = dependency_graph(&m, "src/app.ts", &entry);
        assert!(
            !local.contains(&"src/authentication/index.ts".to_string()),
            "./auth must not resolve to authentication/index.ts (different directory name)"
        );
    }

    #[test]
    fn js_directory_import_downstream_detection() {
        // When `module/index.ts` is resolved from `./module`, it should appear
        // as upstream and session.ts should appear as downstream of the index file.
        let m = manifest_with(vec![
            ("src/auth/module/index.ts", vec![]),
            ("src/auth/session.ts", vec!["./module"]),
        ]);
        let entry = m.files["src/auth/module/index.ts"].clone();
        let (_, _, downstream) = dependency_graph(&m, "src/auth/module/index.ts", &entry);
        assert!(
            downstream.contains(&&"src/auth/session.ts".to_string()),
            "session.ts should appear as downstream of module/index.ts, got: {:?}",
            downstream
        );
    }

    #[test]
    fn reverse_index_large_manifest_correctness() {
        // Build a 1,000-file manifest and verify the reverse index gives correct results.
        // This guards against the O(N²) regression — if reverse_deps isn't built,
        // downstream returns empty and the assertions fail.
        let hub = "core/base.ts";
        let mut files: Vec<(&str, Vec<&str>)> = vec![(hub, vec![])];
        let paths: Vec<String> = (0..999).map(|i| format!("spoke/file_{}.ts", i)).collect();
        for p in &paths {
            files.push((p.as_str(), vec!["../core/base"]));
        }

        let m = manifest_with(files);
        let entry = m.files[hub].clone();
        let (local, _, downstream) = dependency_graph(&m, hub, &entry);

        assert!(local.is_empty(), "hub has no upstream deps");
        assert_eq!(
            downstream.len(),
            999,
            "all 999 spoke files should appear downstream, got {}",
            downstream.len()
        );

        // Spot check: first and last spoke appear in downstream
        assert!(
            downstream.contains(&&"spoke/file_0.ts".to_string()),
            "spoke/file_0.ts should be downstream"
        );
        assert!(
            downstream.contains(&&"spoke/file_998.ts".to_string()),
            "spoke/file_998.ts should be downstream"
        );

        // Also verify a spoke's upstream contains the hub
        let spoke_entry = m.files["spoke/file_0.ts"].clone();
        let (spoke_local, _, spoke_down) = dependency_graph(&m, "spoke/file_0.ts", &spoke_entry);
        assert!(
            spoke_local.contains(&hub.to_string()),
            "hub should be upstream of spoke, got: {:?}",
            spoke_local
        );
        assert!(spoke_down.is_empty(), "spokes have no downstream");
    }

    // -------------------------------------------------------------------------
    // depends_on extension normalization (ALP-901)
    // -------------------------------------------------------------------------

    #[test]
    fn depends_on_with_extension_equals_without() {
        // Scenario from TanStack report: `depends_on: src/db/schema.ts` returned 1 result
        // while `depends_on: src/db/schema` returned 21. Should be identical.
        let m = manifest_with(vec![
            ("src/db/schema.ts", vec![]),
            ("src/routes/users.ts", vec!["../db/schema"]),
            ("src/routes/posts.ts", vec!["../db/schema.ts"]),
            ("src/services/auth.ts", vec!["../db/schema"]),
        ]);

        let filters_with_ext = SearchFilters {
            export: None,
            imports: None,
            depends_on: Some("src/db/schema.ts".to_string()),
            min_loc: None,
            max_loc: None,
        };
        let filters_without_ext = SearchFilters {
            export: None,
            imports: None,
            depends_on: Some("src/db/schema".to_string()),
            min_loc: None,
            max_loc: None,
        };

        let results_with = filter_search(&m, &filters_with_ext);
        let results_without = filter_search(&m, &filters_without_ext);

        let files_with: Vec<&str> = results_with.iter().map(|r| r.file.as_str()).collect();
        let files_without: Vec<&str> = results_without.iter().map(|r| r.file.as_str()).collect();

        assert_eq!(
            results_with.len(),
            results_without.len(),
            "extension vs no-extension should return same count; with: {:?}, without: {:?}",
            files_with,
            files_without
        );

        for file in &files_with {
            assert!(
                files_without.contains(file),
                "file {:?} in with-ext results but not in without-ext; without: {:?}",
                file,
                files_without
            );
        }

        // All three dependents should appear
        assert!(
            files_with.contains(&"src/routes/users.ts"),
            "users.ts (dep ../db/schema) should match; got: {:?}",
            files_with
        );
        assert!(
            files_with.contains(&"src/routes/posts.ts"),
            "posts.ts (dep ../db/schema.ts) should match; got: {:?}",
            files_with
        );
        assert!(
            files_with.contains(&"src/services/auth.ts"),
            "auth.ts (dep ../db/schema) should match; got: {:?}",
            files_with
        );
    }
}
