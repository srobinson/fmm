//! Per-file sidecar YAML formatters: file outline, symbol lookup, dependency graph, read symbol.

use std::collections::{HashMap, HashSet};

use crate::formatter::yaml_escape;
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
            // Collect public methods belonging to this class (prefix "ClassName.")
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

            // Private members for this class (only when include_private requested)
            let private_members: &[PrivateMember] = private_by_class
                .and_then(|m| m.get(name.as_str()))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            if let Some(el) = entry.export_lines.as_ref().and_then(|els| els.get(i)) {
                let size = el.end.saturating_sub(el.start) + 1;
                let private_count = private_members.len();

                match (class_methods.is_empty(), private_count) {
                    (true, 0) => {
                        lines.push(format!(
                            "  {}: [{}, {}]  # {} lines",
                            yaml_escape(name),
                            el.start,
                            el.end,
                            size
                        ));
                    }
                    (false, 0) => {
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
                    (true, _) => {
                        lines.push(format!(
                            "  {}: [{}, {}]  # {} lines, {} private members",
                            yaml_escape(name),
                            el.start,
                            el.end,
                            size,
                            private_count
                        ));
                        for pm in private_members {
                            let suffix = if pm.is_method {
                                "  # private"
                            } else {
                                "  # private field"
                            };
                            lines.push(format!(
                                "    {}: [{}, {}]{}",
                                yaml_escape(&pm.name),
                                pm.start,
                                pm.end,
                                suffix
                            ));
                        }
                    }
                    (false, _) => {
                        let private_method_count =
                            private_members.iter().filter(|m| m.is_method).count();
                        let private_field_count = private_count - private_method_count;
                        let mut summary = format!(
                            "  {}: [{}, {}]  # {} lines, {} public methods, {} private methods",
                            yaml_escape(name),
                            el.start,
                            el.end,
                            size,
                            class_methods.len(),
                            private_method_count
                        );
                        if private_field_count > 0 {
                            summary.push_str(&format!(", {} private fields", private_field_count));
                        }
                        lines.push(summary);

                        // Merge public (by start line) and private, interleaved by line number.
                        // Public methods are sorted by size desc by the collector above; re-sort
                        // by start line for interleaved display.
                        let mut public_sorted = class_methods.clone();
                        public_sorted.sort_by_key(|(_, el)| el.start);

                        // Build a combined list of (start, label, end, suffix)
                        let mut combined: Vec<(usize, String, usize, &str)> = Vec::new();
                        for (method_name, method_lines) in &public_sorted {
                            combined.push((
                                method_lines.start,
                                method_name.clone(),
                                method_lines.end,
                                "",
                            ));
                        }
                        for pm in private_members {
                            let suffix = if pm.is_method {
                                "  # private"
                            } else {
                                "  # private field"
                            };
                            combined.push((pm.start, pm.name.clone(), pm.end, suffix));
                        }
                        combined.sort_by_key(|(start, _, _, _)| *start);

                        for (start, method_name, end, suffix) in &combined {
                            lines.push(format!(
                                "    {}: [{}, {}]{}",
                                yaml_escape(method_name),
                                start,
                                end,
                                suffix
                            ));
                        }
                    }
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
