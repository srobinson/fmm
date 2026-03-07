//! Search result formatters: bare search, filter search, glossary.

use crate::formatter::yaml_escape;
use crate::manifest::GlossaryEntry;
use crate::search::{BareSearchResult, ExportHitCompact, FileSearchResult};

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

    if !result.named_import_hits.is_empty() {
        let mut lines = Vec::new();
        let header = if colored {
            "\x1b[1mNAMED IMPORTS\x1b[0m"
        } else {
            "NAMED IMPORTS"
        };
        lines.push(header.to_string());
        for hit in &result.named_import_hits {
            lines.push(format!("  {} from {}", hit.symbol, hit.source));
            for file in &hit.files {
                lines.push(format!("    {}", file));
            }
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
            let has_any_callers = !src.used_by.is_empty()
                || !src.namespace_callers.is_empty()
                || !src.layer2_namespace_callers.is_empty();
            if !has_any_callers {
                lines.push("    used_by: []".to_string());
            } else {
                if !src.used_by.is_empty() {
                    let items: Vec<String> = src.used_by.iter().map(|s| yaml_escape(s)).collect();
                    lines.push(format!("    used_by: [{}]", items.join(", ")));
                } else {
                    lines.push("    used_by: []".to_string());
                }
                // ALP-865: disclose each namespace-import caller individually so each gets
                // its own namespace alias (files may import under different namespace names).
                for (f, ns) in &src.namespace_callers {
                    lines.push(format!(
                        "    # {} via namespace import ({}.…) — call-site precision unavailable",
                        yaml_escape(f),
                        ns,
                    ));
                }
                // ALP-882: Layer 2 namespace callers (alias unknown at index time).
                for f in &src.layer2_namespace_callers {
                    lines.push(format!(
                        "    # {} — via namespace import (symbol use unverifiable)",
                        yaml_escape(f),
                    ));
                }
            }
            // ALP-882: disclose how many files were filtered out by Layer 2.
            if src.layer2_excluded_count > 0 {
                let src_basename = src.file.rsplit('/').next().unwrap_or(&src.file);
                lines.push(format!(
                    "    # {} additional {} import {} but not this specific symbol",
                    src.layer2_excluded_count,
                    if src.layer2_excluded_count == 1 {
                        "file imports"
                    } else {
                        "files import"
                    },
                    src_basename,
                ));
            }
            // ALP-883: re-export-only files — impacted by rename but not callers.
            if !src.reexport_files.is_empty() {
                lines.push(format!(
                    "    # re-exports only ({} {} — rename required but no call site):",
                    src.reexport_files.len(),
                    if src.reexport_files.len() == 1 {
                        "file"
                    } else {
                        "files"
                    },
                ));
                for f in &src.reexport_files {
                    lines.push(format!("    #   {}", yaml_escape(f)));
                }
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

fn format_export_compact(e: &ExportHitCompact) -> String {
    match e.lines {
        Some([s, end]) if s > 0 => format!("{} [{}, {}]", e.name, s, end),
        _ => e.name.clone(),
    }
}
