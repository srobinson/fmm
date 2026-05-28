//! Shared text formatters for MCP and CLI output.
//!
//! Produces YAML-style output for per-file tools and CLI-style grouped text
//! for search results.

pub(crate) mod helpers;
pub mod list_formatters;
pub mod search_formatters;
pub mod yaml_formatters;

pub use list_formatters::{
    compute_rollup_buckets, format_list_exports_all, format_list_exports_file,
    format_list_exports_pattern, format_list_files, format_list_files_rollup,
};
pub use search_formatters::{
    format_bare_search, format_filter_search, format_glossary, format_similar,
};
pub use yaml_formatters::{
    format_class_redirect, format_dependency_cycles, format_dependency_graph,
    format_dependency_graph_transitive, format_file_outline, format_lookup_export,
    format_read_symbol,
};

/// Collapse all runs of whitespace (incl. newlines) to single spaces and trim.
/// Used to render multi-line stored signatures on a single line.
pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Escape a string for safe inclusion in YAML output.
///
/// Wraps strings that contain YAML special characters in single quotes.
pub fn yaml_escape(s: &str) -> String {
    const SPECIAL: &[char] = &[
        ':', '#', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ];
    if s.is_empty() || s.contains(SPECIAL) {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        s.to_string()
    }
}

/// Emit the canonical YAML token for a declaration kind, when present.
pub fn push_kind_line(lines: &mut Vec<String>, indent: usize, kind: Option<&str>) {
    if let Some(kind) = kind {
        lines.push(format!("{}kind: {}", " ".repeat(indent), yaml_escape(kind)));
    }
}
