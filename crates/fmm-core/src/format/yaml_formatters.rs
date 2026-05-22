//! Per-file sidecar YAML formatters: file outline, symbol lookup, dependency graph, read symbol.

use std::collections::{HashMap, HashSet};

use crate::format::yaml_escape;
use crate::manifest::private_members::{PrivateMember, TopLevelFunction};
use crate::manifest::{ExportLines, FileEntry, OutlineReExport, SymbolMetadata};

use super::helpers::{push_exports_map, push_inline_list};

/// Format file outline: sidecar YAML with symbol sizes and density metadata.
///
/// `reexports` lists names that this file surfaces from other modules.
/// When non-empty, those names are hidden from the `symbols:` block and
/// rendered in a dedicated `re-exports:` section dereferenced to their
/// origin file + line range.
///
/// `private_by_class` is populated only when `include_private: true` is requested.
/// When `Some`, private members are merged with indexed class members.
/// `top_level_fns` is also populated when `include_private: true` and contains
/// non-exported top-level functions and classes merged into the `symbols:` block.
pub fn format_file_outline(
    file: &str,
    entry: &FileEntry,
    reexports: &[OutlineReExport],
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
    top_level_fns: Option<&[TopLevelFunction]>,
    freshness: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    if let Some(freshness) = freshness {
        lines.push(format!("freshness: {}", yaml_escape(freshness)));
    }
    lines.push(format!("loc: {}", entry.loc));
    push_inline_list(&mut lines, "imports", &entry.imports);
    push_inline_list(&mut lines, "dependencies", &entry.dependencies);

    let reexport_names = reexport_names(reexports);
    let has_local_def = has_local_symbols(entry, &reexport_names)
        || has_extra_top_level_symbols(entry, top_level_fns);

    if has_local_def {
        lines.push("symbols:".to_string());
        push_indexed_symbols(&mut lines, entry, private_by_class, &reexport_names);
        push_extra_top_level_symbols(&mut lines, entry, top_level_fns);
    }

    push_reexports(&mut lines, reexports);
    lines.join("\n")
}

fn reexport_names(reexports: &[OutlineReExport]) -> HashSet<&str> {
    reexports.iter().map(|r| r.name.as_str()).collect()
}

fn has_local_symbols(entry: &FileEntry, reexport_names: &HashSet<&str>) -> bool {
    entry
        .exports
        .iter()
        .any(|name| !reexport_names.contains(name.as_str()))
}

fn has_extra_top_level_symbols(
    entry: &FileEntry,
    top_level_fns: Option<&[TopLevelFunction]>,
) -> bool {
    top_level_fns
        .map(|fns| {
            fns.iter()
                .any(|f| !entry.exports.iter().any(|name| name == &f.name))
        })
        .unwrap_or(false)
}

fn push_indexed_symbols(
    lines: &mut Vec<String>,
    entry: &FileEntry,
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
    reexport_names: &HashSet<&str>,
) {
    for (i, name) in entry.exports.iter().enumerate() {
        if reexport_names.contains(name.as_str()) {
            continue;
        }
        let symbol_lines = entry.export_lines.as_ref().and_then(|els| els.get(i));
        let metadata = entry.export_metadata.get(name);
        push_symbol_entry(lines, 2, name, symbol_lines, metadata);
        push_members(lines, entry, name, private_by_class);
    }
}

fn push_extra_top_level_symbols(
    lines: &mut Vec<String>,
    entry: &FileEntry,
    top_level_fns: Option<&[TopLevelFunction]>,
) {
    let Some(fns) = top_level_fns else {
        return;
    };
    for f in fns {
        if entry.exports.iter().any(|name| name == &f.name) {
            continue;
        }
        let line_range = ExportLines {
            start: f.start,
            end: f.end,
        };
        let metadata = SymbolMetadata {
            signature: None,
            visibility: Some("non_exported".to_string()),
            declaration_kind: Some("fn".to_string()),
        };
        push_symbol_entry(lines, 2, &f.name, Some(&line_range), Some(&metadata));
    }
}

fn push_symbol_entry(
    lines: &mut Vec<String>,
    indent: usize,
    name: &str,
    line_range: Option<&ExportLines>,
    metadata: Option<&SymbolMetadata>,
) {
    lines.push(format!("{}{}:", spaces(indent), yaml_escape(name)));
    if let Some(el) = line_range {
        lines.push(format!(
            "{}lines: [{}, {}]",
            spaces(indent + 2),
            el.start,
            el.end
        ));
        lines.push(format!(
            "{}size: {}",
            spaces(indent + 2),
            el.end.saturating_sub(el.start) + 1
        ));
    }
    if let Some(metadata) = metadata {
        push_metadata(lines, indent + 2, metadata);
    }
}

fn push_metadata(lines: &mut Vec<String>, indent: usize, metadata: &SymbolMetadata) {
    if let Some(signature) = &metadata.signature {
        lines.push(format!(
            "{}signature: {}",
            spaces(indent),
            yaml_escape(signature)
        ));
    }
    if let Some(visibility) = &metadata.visibility {
        lines.push(format!(
            "{}visibility: {}",
            spaces(indent),
            yaml_escape(visibility)
        ));
    }
    crate::format::push_kind_line(lines, indent, metadata.declaration_kind.as_deref());
}

fn push_members(
    lines: &mut Vec<String>,
    entry: &FileEntry,
    parent_name: &str,
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
) {
    let mut members = collect_members(entry, parent_name, private_by_class);
    if members.is_empty() {
        return;
    }

    members.sort_by_key(|member| member.start);
    lines.push(format!("{}members:", spaces(4)));
    for member in members {
        let line_range = ExportLines {
            start: member.start,
            end: member.end,
        };
        push_symbol_entry(
            lines,
            6,
            &member.name,
            Some(&line_range),
            Some(&member.metadata),
        );
    }
}

fn collect_members(
    entry: &FileEntry,
    parent_name: &str,
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
) -> Vec<OutlineMember> {
    let prefix = format!("{}.", parent_name);
    let mut members = Vec::new();
    collect_indexed_members(&mut members, entry.methods.as_ref(), entry, &prefix);
    collect_indexed_members(&mut members, Some(&entry.nested_fns), entry, &prefix);
    if private_by_class.is_some() {
        collect_indexed_members(&mut members, Some(&entry.closure_state), entry, &prefix);
    }
    collect_private_members(&mut members, parent_name, private_by_class);
    members
}

fn collect_indexed_members(
    members: &mut Vec<OutlineMember>,
    source: Option<&HashMap<String, ExportLines>>,
    entry: &FileEntry,
    prefix: &str,
) {
    let Some(source) = source else {
        return;
    };
    for (dotted_name, lines) in source {
        if !dotted_name.starts_with(prefix) {
            continue;
        }
        members.push(OutlineMember {
            name: dotted_name.trim_start_matches(prefix).to_string(),
            start: lines.start,
            end: lines.end,
            metadata: entry
                .method_metadata
                .get(dotted_name)
                .cloned()
                .unwrap_or_default(),
        });
    }
}

fn collect_private_members(
    members: &mut Vec<OutlineMember>,
    parent_name: &str,
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
) {
    let private_members = private_by_class
        .and_then(|m| m.get(parent_name))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    for member in private_members {
        if members.iter().any(|existing| {
            existing.name == member.name
                && existing.start == member.start
                && existing.end == member.end
        }) {
            continue;
        }
        members.push(OutlineMember {
            name: member.name.clone(),
            start: member.start,
            end: member.end,
            metadata: SymbolMetadata {
                signature: None,
                visibility: Some("private".to_string()),
                declaration_kind: Some(
                    if member.is_method { "method" } else { "field" }.to_string(),
                ),
            },
        });
    }
}

fn push_reexports(lines: &mut Vec<String>, reexports: &[OutlineReExport]) {
    if !reexports.is_empty() {
        lines.push("re-exports:".to_string());
        for r in reexports {
            lines.push(format!(
                "  {}: {}:[{}, {}]",
                yaml_escape(&r.name),
                yaml_escape(&r.origin_file),
                r.origin_start,
                r.origin_end
            ));
        }
    }
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}

#[derive(Debug)]
struct OutlineMember {
    name: String,
    start: usize,
    end: usize,
    metadata: SymbolMetadata,
}

/// Format lookup export: sidecar YAML with the found symbol highlighted.
pub fn format_lookup_export(
    symbol: &str,
    file: &str,
    symbol_lines: Option<&ExportLines>,
    entry: &FileEntry,
    collision_note: Option<&str>,
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
    if let Some(note) = collision_note {
        lines.push(String::new());
        lines.push(format!("# {}", note));
    }
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
        let local_set: HashSet<&str> = local.iter().map(|s| s.as_str()).collect();
        lines.push("downstream:".to_string());
        for dep in downstream {
            if local_set.contains(dep.as_str()) {
                lines.push(format!("  - {}  # circular", yaml_escape(dep)));
            } else {
                lines.push(format!("  - {}", yaml_escape(dep)));
            }
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
        let upstream_set: HashSet<&str> = upstream.iter().map(|(p, _)| p.as_str()).collect();
        lines.push("downstream:".to_string());
        for (path, d) in downstream {
            if upstream_set.contains(path.as_str()) {
                lines.push(format!(
                    "  - file: {}  depth: {}  # circular",
                    yaml_escape(path),
                    d
                ));
            } else {
                lines.push(format!("  - file: {}  depth: {}", yaml_escape(path), d));
            }
        }
    }

    push_inline_list(&mut lines, "imports", &entry.imports);
    lines.join("\n")
}

pub fn format_dependency_cycles(cycles: &[Vec<String>]) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push("cycles:".to_string());
    for cycle in cycles {
        lines.push("  - files:".to_string());
        for path in cycle {
            lines.push(format!("      - {}", yaml_escape(path)));
        }
    }
    lines.join("\n")
}

/// Format read symbol: YAML header + source code.
///
/// When `line_numbers` is true, each source line is prefixed with its absolute
/// line number (right-aligned to the width of the last line number). Default: false.
pub fn format_read_symbol(
    symbol: &str,
    file: &str,
    el: &ExportLines,
    kind: Option<&str>,
    source: &str,
    line_numbers: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("symbol: {}", yaml_escape(symbol)));
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!("lines: [{}, {}]", el.start, el.end));
    crate::format::push_kind_line(&mut lines, 0, kind);
    lines.push("---".to_string());

    if line_numbers {
        let source_lines: Vec<&str> = source.lines().collect();
        let last_line = el.start + source_lines.len().saturating_sub(1);
        let width = last_line.to_string().len();
        for (i, line) in source_lines.iter().enumerate() {
            let lineno = el.start + i;
            lines.push(format!("{:>width$}  {}", lineno, line));
        }
    } else {
        lines.push(source.to_string());
    }

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

#[cfg(test)]
#[path = "yaml_formatters_tests.rs"]
mod tests;
