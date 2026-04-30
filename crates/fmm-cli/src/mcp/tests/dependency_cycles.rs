use super::support::{TestServer, assert_error, test_server, tool_text};
use fmm_core::identity::EdgeKind;
use fmm_core::manifest::Manifest;
use fmm_core::parser::Metadata;
use serde_json::json;
use std::collections::HashMap;

fn add_file(
    manifest: &mut Manifest,
    path: &str,
    dependencies: Vec<&str>,
    dependency_kinds: HashMap<String, EdgeKind>,
) {
    manifest.add_file(
        path,
        Metadata {
            dependencies: dependencies.into_iter().map(str::to_string).collect(),
            dependency_kinds,
            loc: 10,
            ..Default::default()
        },
    );
}

fn cycle_server() -> TestServer {
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/a.ts", vec!["./b"], HashMap::new());
    add_file(&mut manifest, "src/b.ts", vec!["./a"], HashMap::new());
    add_file(
        &mut manifest,
        "src/types-a.ts",
        vec!["./types-b"],
        HashMap::from([("./types-b".to_string(), EdgeKind::TypeOnly)]),
    );
    add_file(
        &mut manifest,
        "src/types-b.ts",
        vec!["./types-a"],
        HashMap::from([("./types-a".to_string(), EdgeKind::TypeOnly)]),
    );
    add_file(
        &mut manifest,
        "src/a.test.ts",
        vec!["./test/helper"],
        HashMap::new(),
    );
    add_file(
        &mut manifest,
        "src/test/helper.ts",
        vec!["../a.test"],
        HashMap::new(),
    );
    add_file(
        &mut manifest,
        "src/mixed.ts",
        vec!["./mixed.test"],
        HashMap::new(),
    );
    add_file(
        &mut manifest,
        "src/mixed.test.ts",
        vec!["./mixed"],
        HashMap::new(),
    );
    manifest.rebuild_file_identity().unwrap();
    test_server(manifest)
}

#[test]
fn dependency_cycles_reports_runtime_cycles_by_default() {
    let server = cycle_server();
    let text = tool_text(&server, "fmm_dependency_cycles", json!({}));

    assert!(text.contains("cycles:"));
    assert!(text.contains("src/a.ts"));
    assert!(text.contains("src/b.ts"));
    assert!(!text.contains("src/types-a.ts"), "got:\n{text}");
}

#[test]
fn dependency_cycles_all_edge_mode_includes_type_only_cycles() {
    let server = cycle_server();
    let text = tool_text(
        &server,
        "fmm_dependency_cycles",
        json!({"edge_mode": "all"}),
    );

    assert!(text.contains("src/types-a.ts"), "got:\n{text}");
    assert!(text.contains("src/types-b.ts"), "got:\n{text}");
}

#[test]
fn dependency_cycles_source_filter_excludes_tests() {
    let server = cycle_server();
    let text = tool_text(
        &server,
        "fmm_dependency_cycles",
        json!({"filter": "source"}),
    );

    assert!(text.contains("src/a.ts"), "got:\n{text}");
    assert!(!text.contains("src/a.test.ts"), "got:\n{text}");
    assert!(!text.contains("src/test/helper.ts"), "got:\n{text}");
    assert!(!text.contains("src/mixed.ts"), "got:\n{text}");
}

#[test]
fn dependency_cycles_tests_filter_shows_only_tests() {
    let server = cycle_server();
    let text = tool_text(&server, "fmm_dependency_cycles", json!({"filter": "tests"}));

    assert!(text.contains("src/a.test.ts"), "got:\n{text}");
    assert!(text.contains("src/test/helper.ts"), "got:\n{text}");
    assert!(!text.contains("src/a.ts"), "got:\n{text}");
    assert!(!text.contains("src/mixed.test.ts"), "got:\n{text}");
}

#[test]
fn dependency_cycles_invalid_edge_mode_returns_error() {
    let server = cycle_server();
    let text = tool_text(
        &server,
        "fmm_dependency_cycles",
        json!({"edge_mode": "bad"}),
    );

    assert_error(&text);
}
