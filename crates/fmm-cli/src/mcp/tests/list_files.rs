use super::support::{TestServer, assert_error, list_files_order, test_server, tool_text};
use fmm_core::manifest::{FileEntry, Manifest};
use fmm_core::parser::{ExportEntry, Metadata};
use serde_json::{Value, json};

fn file(path: &str, loc: usize, exports: &[&str]) -> (String, Metadata) {
    (
        path.to_string(),
        Metadata {
            exports: exports
                .iter()
                .map(|name| ExportEntry::new((*name).to_string(), 1, loc.max(1)))
                .collect(),
            loc,
            ..Default::default()
        },
    )
}

fn manifest_from(files: &[(&str, usize, &[&str])]) -> Manifest {
    let mut manifest = Manifest::new();
    for (path, loc, exports) in files {
        let (path, metadata) = file(path, *loc, exports);
        manifest.add_file(&path, metadata);
    }
    manifest
}

fn sort_server() -> TestServer {
    test_server(manifest_from(&[
        ("src/alpha.ts", 100, &["A", "B"]),
        ("src/beta.ts", 30, &["C"]),
        ("src/gamma.ts", 60, &["D", "E", "F"]),
    ]))
}

fn group_by_directory_server() -> TestServer {
    test_server(manifest_from(&[
        ("packages/core/injector/injector.ts", 200, &["X"]),
        ("packages/core/middleware/middleware.ts", 150, &["X"]),
        ("packages/common/decorators/module.ts", 80, &["X"]),
        ("packages/common/interfaces/index.ts", 40, &["X"]),
        ("packages/microservices/client.ts", 120, &["X"]),
    ]))
}

fn source_test_server() -> TestServer {
    test_server(manifest_from(&[
        ("src/app.service.ts", 20, &["AppService"]),
        ("src/app.spec.ts", 15, &[]),
    ]))
}

fn modified_server() -> TestServer {
    let mut manifest = Manifest::new();
    for (path, export, loc, modified) in [
        ("src/alpha.ts", "A", 100, "2026-03-01"),
        ("src/beta.ts", "B", 30, "2026-03-05"),
        ("src/gamma.ts", "C", 60, "2026-02-20"),
    ] {
        manifest.files.insert(
            path.to_string(),
            FileEntry {
                exports: vec![export.to_string()],
                export_lines: None,
                methods: None,
                imports: vec![],
                dependencies: vec![],
                loc,
                modified: Some(modified.to_string()),
                function_names: Vec::new(),
                ..Default::default()
            },
        );
    }
    test_server(manifest)
}

fn assert_order(args: Value, expected: &[&str]) {
    let server = sort_server();
    let actual = list_files_order(&server, args);
    let expected: Vec<String> = expected.iter().map(|path| (*path).to_string()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn list_files_tool_no_args() {
    let server = sort_server();
    let text = tool_text(&server, "fmm_list_files", json!({}));

    assert!(text.contains("total: 3"), "expected total: 3, got: {text}");
    assert!(text.contains("src/alpha.ts"));
    assert!(text.contains("src/beta.ts"));
    assert!(text.contains("src/gamma.ts"));
}

#[test]
fn list_files_tool_with_directory() {
    let server = test_server(manifest_from(&[
        ("src/cli/mod.rs", 30, &["Cli"]),
        ("src/mcp/mod.rs", 100, &[]),
    ]));

    let text = tool_text(&server, "fmm_list_files", json!({"directory": "src/cli/"}));

    assert!(text.contains("total: 1"), "got: {text}");
    assert!(text.contains("src/cli/mod.rs"));
    assert!(!text.contains("src/mcp/mod.rs"));
}

#[test]
fn list_files_tool_pagination_limit_and_offset() {
    let mut manifest = Manifest::new();
    for i in 1..=5 {
        manifest.add_file(
            &format!("src/mod{i}.rs"),
            Metadata {
                exports: vec![ExportEntry::new(format!("Item{i}"), 1, 5)],
                loc: 10,
                ..Default::default()
            },
        );
    }
    let server = test_server(manifest);

    let first = tool_text(
        &server,
        "fmm_list_files",
        json!({"limit": 2, "offset": 0, "sort_by": "name"}),
    );
    assert!(first.contains("total: 5"), "total wrong; got: {first}");
    assert!(first.contains("showing: 1-2 of 5"));
    assert!(first.contains("offset=2"));
    assert!(first.contains("src/mod1.rs"));
    assert!(first.contains("src/mod2.rs"));
    assert!(!first.contains("src/mod3.rs"));

    let second = tool_text(
        &server,
        "fmm_list_files",
        json!({"limit": 2, "offset": 2, "sort_by": "name"}),
    );
    assert!(second.contains("src/mod3.rs"));
    assert!(second.contains("src/mod4.rs"));
    assert!(!second.contains("src/mod1.rs"));

    let last = tool_text(
        &server,
        "fmm_list_files",
        json!({"limit": 2, "offset": 4, "sort_by": "name"}),
    );
    assert!(last.contains("src/mod5.rs"));
    assert!(!last.contains("offset=6"));

    let out_of_bounds = tool_text(
        &server,
        "fmm_list_files",
        json!({"limit": 2, "offset": 1000}),
    );
    assert!(out_of_bounds.contains("total: 5"));
    assert!(!out_of_bounds.contains("showing:"));
}

#[test]
fn list_files_sorting_orders_match_contract() {
    for (args, expected) in [
        (
            json!({}),
            vec!["src/alpha.ts", "src/gamma.ts", "src/beta.ts"],
        ),
        (
            json!({"sort_by": "loc"}),
            vec!["src/alpha.ts", "src/gamma.ts", "src/beta.ts"],
        ),
        (
            json!({"sort_by": "loc", "order": "asc"}),
            vec!["src/beta.ts", "src/gamma.ts", "src/alpha.ts"],
        ),
        (
            json!({"sort_by": "exports"}),
            vec!["src/gamma.ts", "src/alpha.ts", "src/beta.ts"],
        ),
        (
            json!({"sort_by": "name", "order": "desc"}),
            vec!["src/gamma.ts", "src/beta.ts", "src/alpha.ts"],
        ),
        (
            json!({"sort_by": "path"}),
            vec!["src/alpha.ts", "src/beta.ts", "src/gamma.ts"],
        ),
    ] {
        assert_order(args, &expected);
    }
}

#[test]
fn list_files_rejects_invalid_sort_order_and_grouping() {
    for args in [
        json!({"sort_by": "invalid"}),
        json!({"sort_by": "loc", "order": "random"}),
        json!({"group_by": "unknown"}),
    ] {
        let server = sort_server();
        let text = tool_text(&server, "fmm_list_files", args);
        assert_error(&text);
    }
}

#[test]
fn list_files_group_by_subdir_buckets_files_by_immediate_dir() {
    let server = sort_server();
    let text = tool_text(&server, "fmm_list_files", json!({"group_by": "subdir"}));

    assert!(
        text.contains("src/"),
        "should show src/ bucket; got:\n{text}"
    );
    assert!(
        text.contains("3 files"),
        "bucket should show 3 files; got:\n{text}"
    );
    assert!(
        text.contains("190 LOC"),
        "bucket should show 190 total LOC; got:\n{text}"
    );
    assert!(
        text.contains("summary:"),
        "summary line should appear; got:\n{text}"
    );
}

#[test]
fn list_files_directory_dot_returns_all_files() {
    let server = sort_server();
    let dot = tool_text(&server, "fmm_list_files", json!({"directory": "."}));
    let none = tool_text(&server, "fmm_list_files", json!({}));

    assert_eq!(dot, none, "directory='.' must match omitted directory");
}

#[test]
fn list_files_directory_dot_slash_returns_all_files() {
    let server = sort_server();
    let text = tool_text(&server, "fmm_list_files", json!({"directory": "./"}));

    assert!(text.contains("src/alpha.ts"), "got:\n{text}");
    assert!(text.contains("total: 3"), "got:\n{text}");
}

#[test]
fn list_files_invalid_directory_returns_empty() {
    let server = sort_server();
    let text = tool_text(
        &server,
        "fmm_list_files",
        json!({"directory": "doesnotexist"}),
    );

    assert!(text.contains("total: 0"), "got:\n{text}");
}

#[test]
fn list_files_group_by_subdir_with_directory_splits_into_subdirs() {
    let server = group_by_directory_server();
    let text = tool_text(
        &server,
        "fmm_list_files",
        json!({"directory": "packages", "group_by": "subdir"}),
    );

    for bucket in [
        "packages/core/",
        "packages/common/",
        "packages/microservices/",
    ] {
        assert!(text.contains(bucket), "missing {bucket}; got:\n{text}");
    }
}

#[test]
fn list_files_group_by_subdir_no_directory_unchanged() {
    let server = group_by_directory_server();
    let text = tool_text(&server, "fmm_list_files", json!({"group_by": "subdir"}));

    assert!(text.contains("packages/"), "got:\n{text}");
    assert!(
        !text.contains("packages/core/"),
        "top level rollup should not show child buckets; got:\n{text}",
    );
}

#[test]
fn list_files_group_by_subdir_nested_directory() {
    let server = group_by_directory_server();
    let text = tool_text(
        &server,
        "fmm_list_files",
        json!({"directory": "packages/core", "group_by": "subdir"}),
    );

    assert!(text.contains("packages/core/injector/"), "got:\n{text}");
    assert!(text.contains("packages/core/middleware/"), "got:\n{text}");
}

#[test]
fn list_files_source_test_filtering() {
    let server = source_test_server();

    let source = tool_text(&server, "fmm_list_files", json!({"filter": "source"}));
    assert!(source.contains("src/app.service.ts"), "got:\n{source}");
    assert!(!source.contains("src/app.spec.ts"), "got:\n{source}");
    assert!(source.contains("total: 1"), "got:\n{source}");

    let tests = tool_text(&server, "fmm_list_files", json!({"filter": "tests"}));
    assert!(tests.contains("src/app.spec.ts"), "got:\n{tests}");
    assert!(!tests.contains("src/app.service.ts"), "got:\n{tests}");
    assert!(tests.contains("total: 1"), "got:\n{tests}");

    let invalid = tool_text(&server, "fmm_list_files", json!({"filter": "bogus"}));
    assert_error(&invalid);
}

#[test]
fn list_files_sort_by_modified() {
    let server = modified_server();

    let desc = list_files_order(&server, json!({"sort_by": "modified"}));
    assert_eq!(
        desc,
        vec!["src/beta.ts", "src/alpha.ts", "src/gamma.ts"],
        "modified should default to most recent first",
    );

    let asc = list_files_order(&server, json!({"sort_by": "modified", "order": "asc"}));
    assert_eq!(
        asc,
        vec!["src/gamma.ts", "src/alpha.ts", "src/beta.ts"],
        "modified asc should return oldest first",
    );

    let text = tool_text(&server, "fmm_list_files", json!({"sort_by": "modified"}));
    assert!(
        text.contains("modified: 2026-03-05"),
        "output should show modified date, got:\n{text}",
    );

    let filtered = list_files_order(&server, json!({"sort_by": "modified", "directory": "src/"}));
    assert_eq!(
        filtered,
        vec!["src/beta.ts", "src/alpha.ts", "src/gamma.ts"]
    );
}
