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

/// Format file outline: sidecar YAML with symbol sizes.
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
            if let Some(el) = entry.export_lines.as_ref().and_then(|els| els.get(i)) {
                let size = el.end.saturating_sub(el.start) + 1;
                lines.push(format!(
                    "  {}: [{}, {}]  # {} lines",
                    yaml_escape(name),
                    el.start,
                    el.end,
                    size
                ));
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

// ---------------------------------------------------------------------------
// List exports formatters
// ---------------------------------------------------------------------------

/// Format list exports for a pattern search: column-aligned text.
pub fn format_list_exports_pattern(matches: &[(String, String, Option<[usize; 2]>)]) -> String {
    if matches.is_empty() {
        return String::new();
    }
    let name_width = matches.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    let file_width = matches.iter().map(|(_, f, _)| f.len()).max().unwrap_or(0);

    let mut out = Vec::new();
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

/// Format list exports for all files: multi-document sidecar YAML.
pub fn format_list_exports_all(files: &[(&str, &FileEntry)]) -> String {
    let mut docs = Vec::new();
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

/// Format list files result as compact YAML.
/// Each entry shows: file path, loc, export count.
pub fn format_list_files(directory: Option<&str>, files: &[(&str, usize, usize)]) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if let Some(dir) = directory {
        lines.push(format!("directory: {}", yaml_escape(dir)));
    }
    lines.push(format!("total: {}", files.len()));
    if !files.is_empty() {
        lines.push("files:".to_string());
        // Column width for alignment
        let path_width = files.iter().map(|(p, _, _)| p.len()).max().unwrap_or(0);
        for (path, loc, exports) in files {
            lines.push(format!(
                "  - {:<pw$}  # loc: {}, exports: {}",
                path,
                loc,
                exports,
                pw = path_width,
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
pub fn format_filter_search(results: &[FileSearchResult], colored: bool) -> String {
    let mut out = Vec::new();
    for r in results {
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
