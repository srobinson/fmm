use super::support::{assert_error, test_server, tool_text};
use fmm_core::manifest::Manifest;
use serde_json::json;

#[test]
fn dependency_graph_directory_path_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_dependency_graph", json!({"file": "src/mcp/"}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {text}",
    );
}

#[test]
fn read_symbol_empty_name_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_read_symbol", json!({"name": ""}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_exports"),
        "should suggest fmm_list_exports, got: {text}",
    );
}

#[test]
fn file_outline_directory_path_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_file_outline", json!({"file": "src/cli/"}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {text}",
    );
}
