use super::support::{TestServer, assert_error, test_server, tool_text};
use fmm_core::manifest::Manifest;
use fmm_core::parser::Metadata;
use serde_json::json;

fn dependency_filter_server() -> TestServer {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/core.ts",
        Metadata {
            loc: 100,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/service.ts",
        Metadata {
            dependencies: vec!["./core".to_string()],
            loc: 80,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/core.spec.ts",
        Metadata {
            dependencies: vec!["./core".to_string()],
            loc: 50,
            ..Default::default()
        },
    );
    manifest.rebuild_reverse_deps();
    test_server(manifest)
}

#[test]
fn dependency_graph_filter_all_is_default() {
    let server = dependency_filter_server();
    let text = tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/core.ts"}),
    );

    assert!(
        text.contains("src/service.ts"),
        "should show source downstream; got:\n{text}",
    );
    assert!(
        text.contains("src/core.spec.ts"),
        "should show test downstream without filter; got:\n{text}",
    );
}

#[test]
fn dependency_graph_filter_source_excludes_tests() {
    let server = dependency_filter_server();
    let text = tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/core.ts", "filter": "source"}),
    );

    assert!(
        text.contains("src/service.ts"),
        "source filter should keep src/service.ts; got:\n{text}",
    );
    assert!(
        !text.contains("src/core.spec.ts"),
        "source filter must exclude src/core.spec.ts; got:\n{text}",
    );
}

#[test]
fn dependency_graph_filter_tests_shows_only_tests() {
    let server = dependency_filter_server();
    let text = tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/core.ts", "filter": "tests"}),
    );

    assert!(
        text.contains("src/core.spec.ts"),
        "tests filter should show src/core.spec.ts; got:\n{text}",
    );
    assert!(
        !text.contains("src/service.ts"),
        "tests filter must exclude src/service.ts; got:\n{text}",
    );
}

#[test]
fn dependency_graph_invalid_filter_returns_error() {
    let server = dependency_filter_server();
    let text = tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/core.ts", "filter": "bad"}),
    );

    assert_error(&text);
}
