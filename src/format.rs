//! Shared text formatters for MCP and CLI output.
//!
//! Produces `.fmm`-style sidecar YAML for per-file tools and
//! CLI-style grouped text for search results.

use crate::formatter::yaml_escape;
use crate::manifest::{ExportLines, FileEntry, GlossaryEntry};
use crate::search::{BareSearchResult, ExportHitCompact, FileSearchResult};

// ---------------------------------------------------------------------------
// Per-file sidecar YAML formatters
// ---------------------------------------------------------------------------

/// Format file info as sidecar YAML (exact .fmm format without version/modified).
pub fn format_file_info(file: &str, entry: &FileEntry) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    push_exports_map(&mut lines, &entry.exports, entry.export_lines.as_ref());
    push_inline_list(&mut lines, "imports", &entry.imports);
    push_inline_list(&mut lines, "dependencies", &entry.dependencies);
    lines.push(format!("loc: {}", entry.loc));
    lines.join("\n")
}

/// Format file outline: sidecar YAML with symbol sizes and method sub-entries.
pub fn format_file_outline(file: &str, entry: &FileEntry) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!("loc: {}", entry.loc));
    push_inline_list(&mut lines, "imports", &entry.imports);
    push_inline_list(&mut lines, "dependencies", &entry.dependencies);

    if !entry.exports.is_empty() {
        lines.push("symbols:".to_string());
        for (i, name) in entry.exports.iter().enumerate() {
            // Collect methods belonging to this class (prefix "ClassName.")
            let class_methods: Vec<_> = entry
                .methods
                .as_ref()
                .map(|m| {
                    let prefix = format!("{}.", name);
                    let mut v: Vec<_> = m
                        .iter()
                        .filter(|(k, _)| k.starts_with(&prefix))
                        .map(|(k, v)| (k.trim_start_matches(&prefix).to_string(), v))
                        .collect();
                    v.sort_by(|a, b| {
                        let a_size = a.1.end.saturating_sub(a.1.start);
                        let b_size = b.1.end.saturating_sub(b.1.start);
                        b_size.cmp(&a_size)
                    });
                    v
                })
                .unwrap_or_default();

            if let Some(el) = entry.export_lines.as_ref().and_then(|els| els.get(i)) {
                let size = el.end.saturating_sub(el.start) + 1;
                if class_methods.is_empty() {
                    lines.push(format!(
                        "  {}: [{}, {}]  # {} lines",
                        yaml_escape(name),
                        el.start,
                        el.end,
                        size
                    ));
                } else {
                    lines.push(format!(
                        "  {}: [{}, {}]  # {} lines, {} public methods",
                        yaml_escape(name),
                        el.start,
                        el.end,
                        size,
                        class_methods.len()
                    ));
                    for (method_name, method_lines) in &class_methods {
                        lines.push(format!(
                            "    {}: [{}, {}]",
                            yaml_escape(method_name),
                            method_lines.start,
                            method_lines.end
                        ));
                    }
                }
            } else {
                lines.push(format!("  {}", yaml_escape(name)));
            }
        }
    }
    lines.join("\n")
}

/// Format lookup export: sidecar YAML with the found symbol highlighted.
pub fn format_lookup_export(
    symbol: &str,
    file: &str,
    symbol_lines: Option<&ExportLines>,
    entry: &FileEntry,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("symbol: {}", yaml_escape(symbol)));
    lines.push(format!("file: {}", yaml_escape(file)));
    if let Some(el) = symbol_lines {
        lines.push(format!("lines: [{}, {}]", el.start, el.end));
    }
    push_exports_map(&mut lines, &entry.exports, entry.export_lines.as_ref());
    push_inline_list(&mut lines, "imports", &entry.imports);
    push_inline_list(&mut lines, "dependencies", &entry.dependencies);
    lines.push(format!("loc: {}", entry.loc));
    lines.join("\n")
}

/// Format dependency graph as YAML.
/// `local` contains resolved intra-project file paths; `external` contains package names.
pub fn format_dependency_graph(
    file: &str,
    entry: &FileEntry,
    local: &[String],
    external: &[String],
    downstream: &[&String],
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));

    if !local.is_empty() {
        let items: Vec<String> = local.iter().map(|s| yaml_escape(s)).collect();
        lines.push(format!("local_deps: [{}]", items.join(", ")));
    }

    if !external.is_empty() {
        let items: Vec<String> = external.iter().map(|s| yaml_escape(s)).collect();
        lines.push(format!("external: [{}]", items.join(", ")));
    }

    if !downstream.is_empty() {
        lines.push("downstream:".to_string());
        for dep in downstream {
            lines.push(format!("  - {}", yaml_escape(dep)));
        }
    }

    push_inline_list(&mut lines, "imports", &entry.imports);
    lines.join("\n")
}

/// Format dependency graph for transitive results (depth > 1 or depth = -1).
///
/// Renders a flat list with `depth:` annotation per entry. The `local_deps`
/// and `downstream` vectors contain `(file, depth_discovered_at)` pairs.
pub fn format_dependency_graph_transitive(
    file: &str,
    entry: &FileEntry,
    upstream: &[(String, i32)],
    external: &[String],
    downstream: &[(String, i32)],
    max_depth: i32,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    if max_depth == -1 {
        lines.push("depth: full (transitive closure)".to_string());
    } else {
        lines.push(format!("depth: {}", max_depth));
    }

    if !upstream.is_empty() {
        lines.push("local_deps:".to_string());
        for (path, d) in upstream {
            lines.push(format!("  - file: {}  depth: {}", yaml_escape(path), d));
        }
    }

    if !external.is_empty() {
        let items: Vec<String> = external.iter().map(|s| yaml_escape(s)).collect();
        lines.push(format!("external: [{}]", items.join(", ")));
    }

    if !downstream.is_empty() {
        lines.push("downstream:".to_string());
        for (path, d) in downstream {
            lines.push(format!("  - file: {}  depth: {}", yaml_escape(path), d));
        }
    }

    push_inline_list(&mut lines, "imports", &entry.imports);
    lines.join("\n")
}

/// Format read symbol: YAML header + source code.
pub fn format_read_symbol(symbol: &str, file: &str, el: &ExportLines, source: &str) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("symbol: {}", yaml_escape(symbol)));
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!("lines: [{}, {}]", el.start, el.end));
    lines.push("---".to_string());
    lines.push(source.to_string());
    lines.join("\n")
}

/// Format a class-redirect response when a bare class read would exceed the 10KB cap.
///
/// Shows the class name, file, line range, size, method count, method list, and redirect hints.
pub fn format_class_redirect(
    symbol: &str,
    file: &str,
    el: &ExportLines,
    methods: &[(&str, &ExportLines)],
) -> String {
    let size = el.end.saturating_sub(el.start) + 1;
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!(
        "# {} would exceed the 10KB response cap ({} lines, {} public methods).",
        symbol,
        size,
        methods.len()
    ));
    lines.push(format!("symbol: {}", yaml_escape(symbol)));
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!(
        "lines: [{}, {}]  # {} lines",
        el.start, el.end, size
    ));
    if !methods.is_empty() {
        let name_width = methods.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        lines.push("methods:".to_string());
        for (name, mel) in methods {
            let msize = mel.end.saturating_sub(mel.start) + 1;
            lines.push(format!(
                "  {:<nw$}  [{}, {}]  # {} lines",
                name,
                mel.start,
                mel.end,
                msize,
                nw = name_width,
            ));
        }
    }
    lines.push("---".to_string());
    if let Some((first_method, _)) = methods.first() {
        lines.push(format!(
            "# Use dotted notation to read a specific method: fmm_read_symbol(\"{}.{}\")",
            symbol, first_method
        ));
    }
    lines.push("# Use truncate: false for full source.".to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// List exports formatters
// ---------------------------------------------------------------------------

/// Format list exports for a pattern search: column-aligned text with optional pagination.
///
/// - `matches`: the current page of results (already sliced by offset/limit)
/// - `total`: total number of matches before pagination
/// - `offset`: the page start index (0-based)
pub fn format_list_exports_pattern(
    matches: &[(String, String, Option<[usize; 2]>)],
    total: usize,
    offset: usize,
) -> String {
    if matches.is_empty() {
        return String::new();
    }
    let name_width = matches.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    let file_width = matches.iter().map(|(_, f, _)| f.len()).max().unwrap_or(0);

    let mut out = Vec::new();
    let showing = matches.len();
    if showing < total {
        let end = offset + showing;
        out.push(format!("# showing: {}-{} of {}", offset + 1, end, total));
        if end < total {
            out.push(format!("# next: Use offset={} to continue.", end));
        }
    }
    for (name, file, lines) in matches {
        let lines_str = match lines {
            Some([s, e]) => format!("  [{}, {}]", s, e),
            None => String::new(),
        };
        out.push(format!(
            "{:<nw$}  {:<fw$}{}",
            name,
            file,
            lines_str,
            nw = name_width,
            fw = file_width,
        ));
    }
    out.join("\n")
}

/// Format list exports for a specific file: sidecar YAML.
pub fn format_list_exports_file(file: &str, entry: &FileEntry) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    push_exports_map(&mut lines, &entry.exports, entry.export_lines.as_ref());
    lines.join("\n")
}

/// Format list exports for all files: multi-document sidecar YAML with optional pagination.
///
/// - `files`: the current page of entries (already sliced by offset/limit)
/// - `total`: total number of matching files before pagination
/// - `offset`: the page start index (0-based)
pub fn format_list_exports_all(
    files: &[(&str, &FileEntry)],
    total: usize,
    offset: usize,
) -> String {
    let mut docs = Vec::new();
    let showing = files.len();
    if showing > 0 && showing < total {
        let end = offset + showing;
        let mut header = Vec::new();
        header.push("---".to_string());
        header.push(format!("showing: {}-{} of {}", offset + 1, end, total));
        if end < total {
            header.push(format!("next: Use offset={} to continue.", end));
        }
        docs.push(header.join("\n"));
    }
    for (file, entry) in files {
        let mut lines = Vec::new();
        lines.push("---".to_string());
        lines.push(format!("file: {}", yaml_escape(file)));

        if entry.exports.is_empty() {
            lines.push("exports: []".to_string());
        } else {
            let items: Vec<String> = entry.exports.iter().map(|s| yaml_escape(s)).collect();
            lines.push(format!("exports: [{}]", items.join(", ")));
        }
        docs.push(lines.join("\n"));
    }
    docs.join("\n")
}

// ---------------------------------------------------------------------------
// List files formatter
// ---------------------------------------------------------------------------

/// Format list files result as compact YAML with optional pagination metadata.
///
/// Entry tuple: `(path, loc, exports, downstream_count, modified)`.
///
/// - `directory`: directory prefix filter, shown in header
/// - `files`: the current page of entries (already sliced by offset/limit)
/// - `total`: total number of matching files before pagination
/// - `total_loc`: sum of LOC across all matching files (full set, not page)
/// - `largest`: path and LOC of the largest file in the full set
/// - `offset`: the page start index (0-based)
/// - `show_modified`: when true, include the modified date in each file row
pub fn format_list_files(
    directory: Option<&str>,
    files: &[(&str, usize, usize, usize, Option<&str>)],
    total: usize,
    total_loc: usize,
    largest: Option<(&str, usize)>,
    offset: usize,
    show_modified: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if let Some(dir) = directory {
        lines.push(format!("directory: {}", yaml_escape(dir)));
    }
    // Summary: file count, total LOC, largest file — scoped to the filtered set
    let summary = match largest {
        Some((path, loc)) => format!(
            "{} files · {} LOC · largest: {} ({} LOC)",
            format_count(total),
            format_count(total_loc),
            path,
            format_count(loc),
        ),
        None => format!(
            "{} files · {} LOC",
            format_count(total),
            format_count(total_loc)
        ),
    };
    lines.push(format!("summary: {}", summary));
    lines.push(format!("total: {}", total));
    let showing = files.len();
    if showing > 0 && showing < total {
        let end = offset + showing;
        lines.push(format!("showing: {}-{} of {}", offset + 1, end, total,));
        if end < total {
            lines.push(format!("next: Use offset={} to continue.", end));
        }
    }
    if !files.is_empty() {
        lines.push("files:".to_string());
        // Column width for alignment
        let path_width = files
            .iter()
            .map(|(p, _, _, _, _)| p.len())
            .max()
            .unwrap_or(0);
        for (path, loc, exports, downstream, modified) in files {
            let downstream_str = if *downstream > 0 {
                format!(", ↓ {} downstream", downstream)
            } else {
                String::new()
            };
            let modified_str = if show_modified {
                match modified {
                    Some(d) => format!(", modified: {}", d),
                    None => String::new(),
                }
            } else {
                String::new()
            };
            lines.push(format!(
                "  - {:<pw$}  # loc: {}, exports: {}{}{}",
                path,
                loc,
                exports,
                downstream_str,
                modified_str,
                pw = path_width,
            ));
        }
    }
    lines.join("\n")
}

/// Compute directory-rollup buckets from a flat file list.
///
/// Groups entries by their immediate subdirectory relative to `prefix`,
/// sums file counts and LOC, and returns sorted `(dir, file_count, total_loc)` triples.
/// `sort_by` can be `"loc"` (default desc) or `"name"` (default asc); `order` overrides.
pub fn compute_rollup_buckets(
    entries: &[(&str, usize, usize)],
    prefix: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
) -> Vec<(String, usize, usize)> {
    use std::collections::HashMap;
    let prefix = prefix.unwrap_or("");
    let mut buckets: HashMap<String, (usize, usize)> = HashMap::new();

    for (path, loc, _) in entries {
        let rel = path.strip_prefix(prefix).unwrap_or(path);
        let bucket = if let Some(idx) = rel.find('/') {
            format!("{}{}/", prefix, &rel[..idx])
        } else if prefix.is_empty() {
            "(root)".to_string()
        } else {
            prefix.to_string()
        };
        let e = buckets.entry(bucket).or_insert((0, 0));
        e.0 += 1;
        e.1 += loc;
    }

    let mut bucket_vec: Vec<(String, usize, usize)> = buckets
        .into_iter()
        .map(|(dir, (count, loc))| (dir, count, loc))
        .collect();

    let desc = match sort_by {
        "name" => order == Some("desc"),
        _ => order != Some("asc"),
    };

    match sort_by {
        "name" => {
            if desc {
                bucket_vec.sort_by(|(a, _, _), (b, _, _)| b.cmp(a));
            } else {
                bucket_vec.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
            }
        }
        _ => {
            if desc {
                bucket_vec.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));
            } else {
                bucket_vec.sort_by(|(_, _, a), (_, _, b)| a.cmp(b));
            }
        }
    }

    bucket_vec
}

/// Format list files in directory-rollup mode (group_by="subdir").
///
/// Each bucket row shows: directory path, file count, total LOC.
/// `buckets` is a sorted slice of `(dir_path, file_count, total_loc)`.
pub fn format_list_files_rollup(
    directory: Option<&str>,
    buckets: &[(String, usize, usize)],
    total_files: usize,
    total_loc: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if let Some(dir) = directory {
        lines.push(format!("directory: {}", yaml_escape(dir)));
    }
    lines.push(format!(
        "summary: {} files · {} LOC",
        format_count(total_files),
        format_count(total_loc),
    ));
    lines.push(format!("buckets: {}", buckets.len()));

    if !buckets.is_empty() {
        let dir_width = buckets.iter().map(|(d, _, _)| d.len()).max().unwrap_or(0);
        let count_width = buckets
            .iter()
            .map(|(_, n, _)| format_count(*n).len())
            .max()
            .unwrap_or(0);
        for (dir, count, loc) in buckets {
            lines.push(format!(
                "  {:<dw$}  {:>cw$} files  · {} LOC",
                dir,
                format_count(*count),
                format_count(*loc),
                dw = dir_width,
                cw = count_width,
            ));
        }
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Search formatters
// ---------------------------------------------------------------------------

/// Format bare search result as CLI grouped text.
/// When `colored` is true, uses ANSI escape codes (for terminal).
/// Shows a truncation notice if results were capped by the limit.
pub fn format_bare_search(result: &BareSearchResult, colored: bool) -> String {
    let mut sections = Vec::new();

    if !result.exports.is_empty() {
        let mut lines = Vec::new();
        let header = if colored {
            "\x1b[1mEXPORTS\x1b[0m"
        } else {
            "EXPORTS"
        };
        lines.push(header.to_string());

        let name_width = result
            .exports
            .iter()
            .map(|e| e.name.len())
            .max()
            .unwrap_or(0);
        let file_width = result
            .exports
            .iter()
            .map(|e| e.file.len())
            .max()
            .unwrap_or(0);

        for hit in &result.exports {
            let lines_str = match hit.lines {
                Some([s, e]) => format!("  [{}, {}]", s, e),
                None => String::new(),
            };
            lines.push(format!(
                "  {:<nw$}  {:<fw$}{}",
                hit.name,
                hit.file,
                lines_str,
                nw = name_width,
                fw = file_width,
            ));
        }
        sections.push(lines.join("\n"));
    }

    if !result.files.is_empty() {
        let mut lines = Vec::new();
        let header = if colored {
            "\x1b[1mFILES\x1b[0m"
        } else {
            "FILES"
        };
        lines.push(header.to_string());
        for path in &result.files {
            lines.push(format!("  {}", path));
        }
        sections.push(lines.join("\n"));
    }

    if !result.imports.is_empty() {
        let mut lines = Vec::new();
        let header = if colored {
            "\x1b[1mIMPORTS\x1b[0m"
        } else {
            "IMPORTS"
        };
        lines.push(header.to_string());
        for hit in &result.imports {
            let file_list: Vec<&str> = hit.files.iter().map(|s| s.as_str()).collect();
            lines.push(format!("  {}  ({})", hit.package, file_list.join(", ")));
        }
        sections.push(lines.join("\n"));
    }

    // Truncation notice if fuzzy results were capped
    if let Some(total_fuzzy) = result.total_exports {
        sections.push(format!(
            "[{} fuzzy matches — showing top {} by relevance. Use a more specific term or set limit.]",
            total_fuzzy,
            result.exports.len(),
        ));
    }

    sections.join("\n\n")
}

/// Format filter search results as CLI per-file detail text.
/// Results are sorted by LOC descending (largest files first).
pub fn format_filter_search(results: &[FileSearchResult], colored: bool) -> String {
    let mut sorted: Vec<&FileSearchResult> = results.iter().collect();
    sorted.sort_by(|a, b| b.loc.cmp(&a.loc));
    let mut out = Vec::new();
    for r in sorted {
        let file_line = if colored {
            format!("\x1b[1m{}\x1b[0m", r.file)
        } else {
            r.file.clone()
        };
        out.push(file_line);

        if !r.exports.is_empty() {
            let formatted: Vec<String> = r.exports.iter().map(format_export_compact).collect();
            out.push(format!("  exports: {}", formatted.join(", ")));
        }
        if !r.imports.is_empty() {
            out.push(format!("  imports: {}", r.imports.join(", ")));
        }
        if !r.dependencies.is_empty() {
            out.push(format!("  dependencies: {}", r.dependencies.join(", ")));
        }
        out.push(format!("  loc: {}", r.loc));
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn push_exports_map(
    lines: &mut Vec<String>,
    exports: &[String],
    export_lines: Option<&Vec<ExportLines>>,
) {
    if exports.is_empty() {
        return;
    }
    lines.push("exports:".to_string());
    for (i, name) in exports.iter().enumerate() {
        if let Some(el) = export_lines.and_then(|els| els.get(i)) {
            if el.start > 0 {
                lines.push(format!(
                    "  {}: [{}, {}]",
                    yaml_escape(name),
                    el.start,
                    el.end
                ));
                continue;
            }
        }
        lines.push(format!("  {}", yaml_escape(name)));
    }
}

/// Format glossary entries as YAML.
///
/// ```yaml
/// run_dispatch:
///   - src: libs/agno/agno/agent/_run.py [1207-1384]
///     used_by: [libs/agno/agno/team/_run.py, libs/agno/agno/team/_task_tools.py]
/// Config:
///   - src: src/config/index.ts [3-8]
///     used_by: [src/api/routes.ts, src/auth/middleware.ts]
/// ```
pub fn format_glossary(entries: &[GlossaryEntry], total_matched: usize, limit: usize) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if entries.is_empty() {
        lines.push("(no matching exports)".to_string());
        return lines.join("\n");
    }
    for entry in entries {
        lines.push(format!("{}:", yaml_escape(&entry.name)));
        for src in &entry.sources {
            let loc_str = match &src.lines {
                Some(l) if l.start > 0 => format!(" [{}-{}]", l.start, l.end),
                _ => String::new(),
            };
            lines.push(format!("  - src: {}{}", src.file, loc_str));
            if src.used_by.is_empty() {
                lines.push("    used_by: []".to_string());
            } else {
                let items: Vec<String> = src.used_by.iter().map(|s| yaml_escape(s)).collect();
                lines.push(format!("    used_by: [{}]", items.join(", ")));
            }
        }
    }
    if total_matched > limit {
        lines.push(format!(
            "\n# showing {}/{} matches — use a more specific pattern to narrow results",
            limit, total_matched
        ));
    }
    lines.join("\n")
}

/// Format a number with comma thousands separators (e.g. 1234567 → "1,234,567").
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn push_inline_list(lines: &mut Vec<String>, key: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let escaped: Vec<String> = items.iter().map(|s| yaml_escape(s)).collect();
    lines.push(format!("{}: [{}]", key, escaped.join(", ")));
}

fn format_export_compact(e: &ExportHitCompact) -> String {
    match e.lines {
        Some([s, end]) if s > 0 => format!("{} [{}, {}]", e.name, s, end),
        _ => e.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ExportLines, FileEntry};
    use std::collections::HashMap;

    fn make_entry_with_methods(
        exports: Vec<(&str, usize, usize)>,
        methods: Vec<(&str, usize, usize)>,
    ) -> FileEntry {
        let names: Vec<String> = exports.iter().map(|(n, _, _)| n.to_string()).collect();
        let lines: Vec<ExportLines> = exports
            .iter()
            .map(|(_, s, e)| ExportLines { start: *s, end: *e })
            .collect();
        let method_map: HashMap<String, ExportLines> = methods
            .into_iter()
            .map(|(k, s, e)| (k.to_string(), ExportLines { start: s, end: e }))
            .collect();
        FileEntry {
            exports: names,
            export_lines: Some(lines),
            methods: if method_map.is_empty() {
                None
            } else {
                Some(method_map)
            },
            imports: vec![],
            dependencies: vec![],
            loc: 400,
            modified: None,
        }
    }

    #[test]
    fn file_outline_shows_methods_under_class() {
        let entry = make_entry_with_methods(
            vec![("NestFactoryStatic", 43, 381), ("NestFactory", 396, 396)],
            vec![
                ("NestFactoryStatic.create", 55, 89),
                ("NestFactoryStatic.createApplicationContext", 132, 158),
            ],
        );
        let out = format_file_outline("src/factory.ts", &entry);

        // Class line shows method count
        assert!(out.contains("NestFactoryStatic: [43, 381]"));
        assert!(out.contains("2 public methods"));

        // Methods are sub-entries (4-space indent)
        assert!(out.contains("    create: [55, 89]"));
        assert!(out.contains("    createApplicationContext: [132, 158]"));

        // Class without methods has no method count annotation
        assert!(out.contains("NestFactory: [396, 396]"));
        assert!(!out.contains("NestFactory.*public methods"));
    }

    #[test]
    fn file_outline_methods_sorted_by_size_descending() {
        let entry = make_entry_with_methods(
            vec![("MyClass", 1, 200)],
            vec![
                ("MyClass.small", 10, 19),    // 9 lines
                ("MyClass.large", 50, 149),   // 99 lines
                ("MyClass.medium", 160, 189), // 29 lines
            ],
        );
        let out = format_file_outline("src/my.ts", &entry);
        let large_pos = out.find("large:").unwrap();
        let medium_pos = out.find("medium:").unwrap();
        let small_pos = out.find("small:").unwrap();
        assert!(
            large_pos < medium_pos && medium_pos < small_pos,
            "methods should be sorted by size descending: large > medium > small"
        );
    }

    #[test]
    fn format_count_inserts_commas() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
        assert_eq!(format_count(487341), "487,341");
    }

    #[test]
    fn list_files_summary_header_included() {
        // Two files: alpha (100 LOC, 2 exports, 5 downstream) and beta (30 LOC, 1 export, 0 downstream)
        let files = vec![
            ("src/alpha.ts", 100usize, 2usize, 5usize, None),
            ("src/beta.ts", 30, 1, 0, None),
        ];
        let out = format_list_files(None, &files, 2, 130, Some(("src/alpha.ts", 100)), 0, false);
        assert!(
            out.contains("summary:"),
            "summary line should appear; got:\n{}",
            out
        );
        assert!(
            out.contains("2 files"),
            "summary should show file count; got:\n{}",
            out
        );
        assert!(
            out.contains("130 LOC"),
            "summary should show total LOC; got:\n{}",
            out
        );
        assert!(
            out.contains("largest: src/alpha.ts (100 LOC)"),
            "summary should show largest file; got:\n{}",
            out
        );
        // Row format: downstream shown when > 0
        assert!(out.contains("src/alpha.ts"));
        assert!(out.contains("# loc: 100"));
        assert!(out.contains("↓ 5 downstream")); // alpha has 5 downstream
        assert!(!out.contains("↓ 0")); // beta has 0 downstream — not shown
    }

    #[test]
    fn file_outline_no_methods_unchanged() {
        let entry = make_entry_with_methods(vec![("foo", 1, 10), ("bar", 12, 20)], vec![]);
        let out = format_file_outline("src/mod.ts", &entry);
        assert!(out.contains("  foo: [1, 10]  # 10 lines"));
        assert!(out.contains("  bar: [12, 20]  # 9 lines"));
        assert!(!out.contains("public methods"));
        assert!(!out.contains("    ")); // no sub-indent
    }
}
