use crate::support::{call_tool_expect_error, call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn dependency_graph_upstream_and_downstream() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("local_deps:"), "got: {}", text);
    assert!(text.contains("src/auth/types.ts"), "got: {}", text);
    assert!(text.contains("src/config.ts"), "got: {}", text);
    assert!(text.contains("imports: [jwt, redis]"));
}

#[test]
fn dependency_graph_shows_downstream_dependents() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts"}),
    );

    assert!(text.contains("downstream:"));
    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("src/db/pool.ts"));
}

#[test]
fn dependency_graph_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/nonexistent.ts"}),
    );
    assert!(text.contains("not found"));
}

#[test]
fn dependency_graph_depth2_returns_depth_annotations() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/auth/session.ts", "depth": 2}),
    );
    assert!(
        text.contains("depth: 2"),
        "output should include depth header; got: {text}"
    );
    assert!(
        text.contains("local_deps:"),
        "local_deps section present; got: {text}"
    );
    assert!(
        text.contains("src/auth/types.ts"),
        "types.ts in upstream; got: {text}"
    );
    assert!(
        text.contains("src/config.ts"),
        "config.ts in upstream; got: {text}"
    );
}

#[test]
fn dependency_graph_depth1_is_default_format() {
    let (_tmp, server) = setup_mcp_server();
    let text_default = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts"}),
    );
    let text_explicit = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts", "depth": 1}),
    );
    assert_eq!(text_default, text_explicit, "depth=1 matches default");
    assert!(
        !text_default.contains("depth:"),
        "depth=1 format has no depth annotation; got: {text_default}"
    );
}
