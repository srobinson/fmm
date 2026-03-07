//! Per-file sidecar YAML formatters: file outline, symbol lookup, dependency graph, read symbol.

use std::collections::{HashMap, HashSet};

use crate::format::yaml_escape;
use crate::manifest::private_members::{PrivateMember, TopLevelFunction};
use crate::manifest::{ExportLines, FileEntry};

use super::helpers::{push_exports_map, push_inline_list};

/// Format file outline: sidecar YAML with symbol sizes and method sub-entries.
/// `private_by_class` is populated only when `include_private: true` is requested.
/// When `Some`, private members are merged with public methods and annotated `# private`.
/// `top_level_fns` is also populated when `include_private: true` and contains
/// non-exported top-level functions and classes, appended after the `symbols:` block.
pub fn format_file_outline(
    file: &str,
    entry: &FileEntry,
    private_by_class: Option<&HashMap<String, Vec<PrivateMember>>>,
    top_level_fns: Option<&[TopLevelFunction]>,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!("loc: {}", entry.loc));
    push_inline_list(&mut lines, "imports", &entry.imports);
    push_inline_list(&mut lines, "dependencies", &entry.dependencies);

    if !entry.exports.is_empty() {
        lines.push("symbols:".to_string());
        for (i, name) in entry.exports.iter().enumerate() {
            let prefix = format!("{}.", name);

            // Collect public methods belonging to this class (prefix "ClassName.")
            let class_methods: Vec<_> = entry
                .methods
                .as_ref()
                .map(|m| {
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

            // ALP-922: nested function declarations (depth-1) under this function
            let mut nested_fn_list: Vec<_> = entry
                .nested_fns
                .iter()
                .filter(|(k, _)| k.starts_with(&prefix))
                .map(|(k, v)| (k.trim_start_matches(&prefix).to_string(), v))
                .collect();
            nested_fn_list.sort_by_key(|(_, el)| el.start);

            // ALP-922: closure-state vars — only when include_private requested
            let include_private_flag = private_by_class.is_some();
            let mut closure_state_list: Vec<_> = if include_private_flag {
                entry
                    .closure_state
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .map(|(k, v)| (k.trim_start_matches(&prefix).to_string(), v))
                    .collect()
            } else {
                Vec::new()
            };
            closure_state_list.sort_by_key(|(_, el)| el.start);

            // Private members for this class (only when include_private requested)
            let private_members: &[PrivateMember] = private_by_class
                .and_then(|m| m.get(name.as_str()))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            if let Some(el) = entry.export_lines.as_ref().and_then(|els| els.get(i)) {
                let size = el.end.saturating_sub(el.start) + 1;
                let private_count = private_members.len();
                let nested_fn_count = nested_fn_list.len();
                let closure_state_count = closure_state_list.len();

                // Build annotation: summarize what sub-entries are present.
                let mut annotation_parts: Vec<String> = Vec::new();
                if !class_methods.is_empty() {
                    annotation_parts.push(format!("{} public methods", class_methods.len()));
                }
                if nested_fn_count > 0 {
                    annotation_parts.push(format!("{} nested functions", nested_fn_count));
                }
                if private_count > 0 {
                    let pm_count = private_members.iter().filter(|m| m.is_method).count();
                    let pf_count = private_count - pm_count;
                    if pm_count > 0 {
                        annotation_parts.push(format!("{} private methods", pm_count));
                    }
                    if pf_count > 0 {
                        annotation_parts.push(format!("{} private fields", pf_count));
                    }
                }
                if include_private_flag && closure_state_count > 0 {
                    annotation_parts.push(format!("{} closure-state", closure_state_count));
                }

                let annotation = if annotation_parts.is_empty() {
                    format!(
                        "  {}: [{}, {}]  # {} lines",
                        yaml_escape(name),
                        el.start,
                        el.end,
                        size
                    )
                } else {
                    format!(
                        "  {}: [{}, {}]  # {} lines, {}",
                        yaml_escape(name),
                        el.start,
                        el.end,
                        size,
                        annotation_parts.join(", ")
                    )
                };
                lines.push(annotation);

                // Sub-entries: build combined list sorted by start line.
                // (start, short_name, end, suffix)
                let mut sub_entries: Vec<(usize, String, usize, &'static str)> = Vec::new();

                // Determine whether interleaving by start line is needed:
                // only when private or nested items are present alongside class methods.
                let needs_start_sort = !private_members.is_empty()
                    || !nested_fn_list.is_empty()
                    || !closure_state_list.is_empty();

                if needs_start_sort {
                    // Mixed sub-entries: sort class methods by start line for interleaving.
                    let mut public_sorted = class_methods.clone();
                    public_sorted.sort_by_key(|(_, el)| el.start);
                    for (method_name, method_lines) in &public_sorted {
                        sub_entries.push((
                            method_lines.start,
                            method_name.clone(),
                            method_lines.end,
                            "",
                        ));
                    }
                } else {
                    // Class methods only: preserve size-descending order (original behaviour).
                    for (method_name, method_lines) in &class_methods {
                        sub_entries.push((
                            method_lines.start,
                            method_name.clone(),
                            method_lines.end,
                            "",
                        ));
                    }
                }

                // Nested functions
                for (fn_name, fn_lines) in &nested_fn_list {
                    sub_entries.push((fn_lines.start, fn_name.clone(), fn_lines.end, ""));
                }

                // Private class members
                for pm in private_members {
                    let suffix = if pm.is_method {
                        "  # private"
                    } else {
                        "  # private field"
                    };
                    sub_entries.push((pm.start, pm.name.clone(), pm.end, suffix));
                }

                // Closure-state vars (only with include_private)
                for (var_name, var_lines) in &closure_state_list {
                    sub_entries.push((
                        var_lines.start,
                        var_name.clone(),
                        var_lines.end,
                        "  # closure-state",
                    ));
                }

                if needs_start_sort {
                    sub_entries.sort_by_key(|(start, _, _, _)| *start);
                }

                for (start, sub_name, end, suffix) in &sub_entries {
                    lines.push(format!(
                        "    {}: [{}, {}]{}",
                        yaml_escape(sub_name),
                        start,
                        end,
                        suffix
                    ));
                }
            } else {
                lines.push(format!("  {}", yaml_escape(name)));
            }
        }
    }

    // ALP-910: Render non-exported top-level functions after the symbols block.
    if let Some(fns) = top_level_fns {
        if !fns.is_empty() {
            lines.push("non_exported:".to_string());
            for f in fns {
                let size = f.end.saturating_sub(f.start) + 1;
                lines.push(format!(
                    "  {}: [{}, {}]  # {} lines  # non-exported",
                    yaml_escape(&f.name),
                    f.start,
                    f.end,
                    size
                ));
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

    // ALP-859: when external imports are present, disclose that they are excluded from
    // the downstream count so the analyst knows to use fmm_search for full reach.
    if !external.is_empty() {
        lines.push("# ℹ Cross-package imports are excluded from the downstream count.".to_string());
        lines.push(
            "#   To find all files that import this path, use: fmm_search(imports=\"<path>\")"
                .to_string(),
        );
    }

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

/// Format read symbol: YAML header + source code.
///
/// When `line_numbers` is true, each source line is prefixed with its absolute
/// line number (right-aligned to the width of the last line number). Default: false.
pub fn format_read_symbol(
    symbol: &str,
    file: &str,
    el: &ExportLines,
    source: &str,
    line_numbers: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("symbol: {}", yaml_escape(symbol)));
    lines.push(format!("file: {}", yaml_escape(file)));
    lines.push(format!("lines: [{}, {}]", el.start, el.end));
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
            function_names: Vec::new(),
            ..Default::default()
        }
    }

    fn make_bare_entry() -> FileEntry {
        FileEntry {
            exports: vec![],
            export_lines: None,
            methods: None,
            imports: vec![],
            dependencies: vec![],
            loc: 50,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
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
        let out = format_file_outline("src/factory.ts", &entry, None, None);

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
        let out = format_file_outline("src/my.ts", &entry, None, None);
        let large_pos = out.find("large:").unwrap();
        let medium_pos = out.find("medium:").unwrap();
        let small_pos = out.find("small:").unwrap();
        assert!(
            large_pos < medium_pos && medium_pos < small_pos,
            "methods should be sorted by size descending: large > medium > small"
        );
    }

    #[test]
    fn file_outline_no_methods_unchanged() {
        let entry = make_entry_with_methods(vec![("foo", 1, 10), ("bar", 12, 20)], vec![]);
        let out = format_file_outline("src/mod.ts", &entry, None, None);
        assert!(out.contains("  foo: [1, 10]  # 10 lines"));
        assert!(out.contains("  bar: [12, 20]  # 9 lines"));
        assert!(!out.contains("public methods"));
        assert!(!out.contains("    ")); // no sub-indent
    }

    #[test]
    fn dependency_graph_no_circular_unchanged() {
        let entry = make_bare_entry();
        let local = vec!["src/a.ts".to_string(), "src/b.ts".to_string()];
        let ds_a = "src/c.ts".to_string();
        let ds_b = "src/d.ts".to_string();
        let downstream = vec![&ds_a, &ds_b];
        let out = format_dependency_graph("src/x.ts", &entry, &local, &[], &downstream);
        assert!(out.contains("  - src/c.ts"));
        assert!(out.contains("  - src/d.ts"));
        assert!(!out.contains("# circular"));
    }

    #[test]
    fn dependency_graph_annotates_circular_downstream() {
        let entry = make_bare_entry();
        // a.ts and b.ts are local deps; b.ts also appears in downstream
        let local = vec!["src/a.ts".to_string(), "src/b.ts".to_string()];
        let ds_b = "src/b.ts".to_string();
        let ds_c = "src/c.ts".to_string();
        let downstream = vec![&ds_b, &ds_c];
        let out = format_dependency_graph("src/x.ts", &entry, &local, &[], &downstream);
        assert!(
            out.contains("  - src/b.ts  # circular"),
            "circular entry missing; got:\n{}",
            out
        );
        assert!(
            out.contains("  - src/c.ts"),
            "non-circular entry wrong; got:\n{}",
            out
        );
    }

    #[test]
    fn dependency_graph_transitive_no_circular_unchanged() {
        let entry = make_bare_entry();
        let upstream = vec![("src/a.ts".to_string(), 1)];
        let downstream = vec![("src/c.ts".to_string(), 1)];
        let out =
            format_dependency_graph_transitive("src/x.ts", &entry, &upstream, &[], &downstream, 1);
        assert!(out.contains("  - file: src/c.ts  depth: 1"));
        assert!(!out.contains("# circular"));
    }

    #[test]
    fn dependency_graph_transitive_annotates_circular_downstream() {
        let entry = make_bare_entry();
        let upstream = vec![("src/a.ts".to_string(), 1), ("src/b.ts".to_string(), 2)];
        let downstream = vec![("src/b.ts".to_string(), 1), ("src/c.ts".to_string(), 1)];
        let out =
            format_dependency_graph_transitive("src/x.ts", &entry, &upstream, &[], &downstream, 2);
        assert!(
            out.contains("  - file: src/b.ts  depth: 1  # circular"),
            "circular entry missing; got:\n{}",
            out
        );
        assert!(
            out.contains("  - file: src/c.ts  depth: 1"),
            "non-circular entry wrong; got:\n{}",
            out
        );
    }

    // ALP-827: private field annotation consistency in mixed public+private case
    #[test]
    fn file_outline_private_field_annotated_correctly_when_public_methods_present() {
        use crate::manifest::private_members::PrivateMember;

        let entry =
            make_entry_with_methods(vec![("MyClass", 1, 50)], vec![("MyClass.doWork", 5, 20)]);

        let mut private_map = HashMap::new();
        private_map.insert(
            "MyClass".to_string(),
            vec![
                PrivateMember {
                    name: "pool".to_string(),
                    start: 3,
                    end: 3,
                    is_method: false,
                },
                PrivateMember {
                    name: "_helper".to_string(),
                    start: 22,
                    end: 30,
                    is_method: true,
                },
            ],
        );

        let out = format_file_outline("src/my.ts", &entry, Some(&private_map), None);

        assert!(
            out.contains("pool: [3, 3]  # private field"),
            "private field should be annotated '# private field'; got:\n{}",
            out
        );
        assert!(
            out.contains("_helper: [22, 30]  # private"),
            "private method should be annotated '# private'; got:\n{}",
            out
        );
        // Confirm the field is NOT just annotated "# private" (without "field")
        assert!(
            !out.contains("pool: [3, 3]  # private\n"),
            "private field must not carry generic '# private' label; got:\n{}",
            out
        );
    }

    // ALP-829: format_read_symbol line_numbers
    #[test]
    fn read_symbol_line_numbers_false_unchanged() {
        let el = ExportLines { start: 10, end: 12 };
        let source = "fn foo() {\n  bar();\n}";
        let out = format_read_symbol("foo", "src/x.rs", &el, source, false);
        assert!(out.contains("fn foo() {"), "source should appear verbatim");
        assert!(!out.contains("10  "), "no line numbers when flag=false");
    }

    #[test]
    fn read_symbol_line_numbers_true_prepends_numbers() {
        let el = ExportLines { start: 10, end: 12 };
        let source = "fn foo() {\n  bar();\n}";
        let out = format_read_symbol("foo", "src/x.rs", &el, source, true);
        assert!(
            out.contains("10  fn foo() {"),
            "line 10 missing; got:\n{}",
            out
        );
        assert!(
            out.contains("11    bar();"),
            "line 11 missing; got:\n{}",
            out
        );
        assert!(out.contains("12  }"), "line 12 missing; got:\n{}", out);
    }

    #[test]
    fn read_symbol_line_numbers_aligned_to_max_width() {
        // Lines 99-101: width=3, so numbers should be right-aligned
        let el = ExportLines {
            start: 99,
            end: 101,
        };
        let source = "a\nb\nc";
        let out = format_read_symbol("x", "f.ts", &el, source, true);
        assert!(
            out.contains(" 99  a"),
            "line 99 not right-aligned; got:\n{}",
            out
        );
        assert!(out.contains("100  b"), "line 100 missing; got:\n{}", out);
        assert!(out.contains("101  c"), "line 101 missing; got:\n{}", out);
    }
}
