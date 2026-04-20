use crate::support::{call_tool_text, setup_go_mcp_server};
use serde_json::json;

#[test]
fn go_internal_import_resolves_upstream() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "cmd/main.go"}),
    );

    assert!(
        text.contains("local_deps:"),
        "expected local_deps in: {}",
        text
    );
    assert!(
        text.contains("internal/handler/handler.go"),
        "expected handler.go as upstream dep, got: {}",
        text
    );
}

#[test]
fn go_internal_import_resolves_downstream() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "internal/handler/handler.go"}),
    );

    assert!(
        text.contains("downstream:"),
        "expected downstream: in: {}",
        text
    );
    assert!(
        text.contains("cmd/main.go"),
        "expected cmd/main.go as downstream dependent, got: {}",
        text
    );
}

#[test]
fn go_stdlib_import_no_false_positive() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "internal/handler/handler.go"}),
    );

    assert!(
        !text.contains("local_deps:"),
        "net/http stdlib import caused false positive local dep: {}",
        text
    );
}
