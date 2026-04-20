use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn search_export_and_imports_both_required() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"export": "Pool", "imports": "pg"}),
    );
    assert!(
        text.contains("src/db/pool.ts"),
        "pool.ts exports Pool AND imports pg; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/auth"),
        "session.ts should not appear because it has no Pool export; got:\n{}",
        text
    );
}

#[test]
fn search_export_and_min_loc_both_required() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"export": "Pool", "min_loc": 50}),
    );
    assert!(
        !text.contains("src/db/pool.ts"),
        "pool.ts with 10 LOC must not appear when min_loc=50; got:\n{}",
        text
    );
}

#[test]
fn search_imports_and_min_loc_both_required() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10}),
    );
    assert!(
        text.contains("src/auth/session.ts"),
        "session.ts matches imports=jwt AND min_loc>=10; got:\n{}",
        text
    );
}

#[test]
fn search_three_filters_and_semantics() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10, "export": "createSession"}),
    );
    assert!(
        text.contains("src/auth/session.ts"),
        "session.ts matches all three filters; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/db"),
        "db files must not appear; got:\n{}",
        text
    );
}

#[test]
fn search_three_filters_one_mismatch_returns_empty() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10, "export": "Pool"}),
    );
    assert!(
        !text.contains("src/"),
        "no file satisfies all three filters; got:\n{}",
        text
    );
}
