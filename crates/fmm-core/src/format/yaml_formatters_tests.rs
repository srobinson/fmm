use super::*;
use crate::manifest::{ExportLines, FileEntry, OutlineReExport};
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
    let out = format_file_outline("src/factory.ts", &entry, &[], None, None);

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
    let out = format_file_outline("src/my.ts", &entry, &[], None, None);
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
    let out = format_file_outline("src/mod.ts", &entry, &[], None, None);
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

    let entry = make_entry_with_methods(vec![("MyClass", 1, 50)], vec![("MyClass.doWork", 5, 20)]);

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

    let out = format_file_outline("src/my.ts", &entry, &[], Some(&private_map), None);

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

// --- Phase 3: re-exports rendering ---

#[test]
fn file_outline_mixed_local_and_reexports() {
    // `main` is local; `BindFailure` and `Manifest` are re-exported.
    let entry = make_entry_with_methods(
        vec![("main", 83, 90), ("BindFailure", 3, 3), ("Manifest", 4, 4)],
        vec![],
    );
    let reexports = vec![
        OutlineReExport {
            name: "BindFailure".to_string(),
            origin_file: "pkg/runner.py".to_string(),
            origin_start: 12,
            origin_end: 30,
        },
        OutlineReExport {
            name: "Manifest".to_string(),
            origin_file: "pkg/manifest.py".to_string(),
            origin_start: 5,
            origin_end: 60,
        },
    ];

    let out = format_file_outline("pkg/__init__.py", &entry, &reexports, None, None);

    // Local def stays in symbols with its in-file line range.
    assert!(
        out.contains("  main: [83, 90]  # 8 lines"),
        "local def must render in symbols; got:\n{}",
        out
    );
    // Re-exported names must NOT appear in the symbols block with their
    // in-file import-line ranges.
    assert!(
        !out.contains("  BindFailure: [3, 3]"),
        "re-export leaked into symbols block; got:\n{}",
        out
    );
    assert!(
        !out.contains("  Manifest: [4, 4]"),
        "re-export leaked into symbols block; got:\n{}",
        out
    );

    // Re-exports section renders with origin file + line range.
    assert!(
        out.contains("re-exports:"),
        "re-exports header missing; got:\n{}",
        out
    );
    assert!(
        out.contains("  BindFailure: pkg/runner.py:[12, 30]"),
        "BindFailure re-export missing; got:\n{}",
        out
    );
    assert!(
        out.contains("  Manifest: pkg/manifest.py:[5, 60]"),
        "Manifest re-export missing; got:\n{}",
        out
    );
}

#[test]
fn file_outline_no_reexports_omits_section() {
    let entry = make_entry_with_methods(vec![("foo", 1, 10), ("bar", 12, 20)], vec![]);
    let out = format_file_outline("src/mod.ts", &entry, &[], None, None);

    assert!(
        !out.contains("re-exports:"),
        "re-exports section must be omitted when empty; got:\n{}",
        out
    );
    assert!(out.contains("  foo: [1, 10]"));
    assert!(out.contains("  bar: [12, 20]"));
}

#[test]
fn file_outline_only_reexports_omits_symbols_block() {
    // A pure re-export file (no local defs) should not emit an empty
    // `symbols:` label above the `re-exports:` section.
    let entry = make_entry_with_methods(vec![("bar", 2, 2)], vec![]);
    let reexports = vec![OutlineReExport {
        name: "bar".to_string(),
        origin_file: "pkg/foo.py".to_string(),
        origin_start: 1,
        origin_end: 3,
    }];

    let out = format_file_outline("pkg/__init__.py", &entry, &reexports, None, None);

    assert!(
        !out.contains("symbols:"),
        "symbols: header must be omitted when no local defs; got:\n{}",
        out
    );
    assert!(
        out.contains("re-exports:"),
        "re-exports header missing; got:\n{}",
        out
    );
    assert!(
        out.contains("  bar: pkg/foo.py:[1, 3]"),
        "re-export entry missing; got:\n{}",
        out
    );
}
