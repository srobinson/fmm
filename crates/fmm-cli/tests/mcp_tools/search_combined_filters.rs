use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn search_term_and_imports_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "session", "imports": "jwt"}),
    );

    assert!(
        text.contains("createSession"),
        "should include createSession"
    );
    assert!(
        text.contains("validateSession"),
        "should include validateSession"
    );
    assert!(
        !text.contains("SessionToken"),
        "SessionToken is in types.ts which doesn't import jwt"
    );
}

#[test]
fn search_term_and_min_loc_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "hashPassword", "min_loc": 10}),
    );

    assert!(
        !text.contains("hashPassword"),
        "hashPassword is in crypto.ts with LOC=9, which fails min_loc=10 filter"
    );
}

#[test]
fn search_term_and_depends_on_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "session", "depends_on": "config"}),
    );

    assert!(
        text.contains("createSession"),
        "session.ts depends on config"
    );
    assert!(
        !text.contains("SessionToken"),
        "types.ts does not depend on config"
    );
}

#[test]
fn search_term_only_regression() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "createSession"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("src/auth/session.ts"));
}

#[test]
fn search_filter_only_regression() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"imports": "jwt"}));

    assert!(text.contains("src/auth/session.ts"));
}
