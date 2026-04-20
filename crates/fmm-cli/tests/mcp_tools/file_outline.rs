use crate::support::{call_tool_expect_error, call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn file_outline_returns_symbols_with_lines() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("symbols:"));
    assert!(text.contains("createSession: [6, 8]"));
    assert!(text.contains("validateSession: [10, 12]"));
    assert!(text.contains("# 3 lines"));
    assert!(text.contains("imports: [jwt, redis]"));
}

#[test]
fn file_outline_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(
        &server,
        "fmm_file_outline",
        json!({"file": "src/nonexistent.ts"}),
    );
    assert!(text.contains("not found"));
}

#[test]
fn file_outline_shows_all_exports() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/utils/crypto.ts"}),
    );

    assert!(text.contains("hashPassword:"));
    assert!(text.contains("verifyPassword:"));
    assert!(text.contains("loc: 9"));
}

#[test]
fn file_outline_returns_symbols() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.starts_with("---"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(
        text.contains("symbols:"),
        "expected symbols: key; got: {text}"
    );
    assert!(text.contains("createSession:"));
    assert!(text.contains("validateSession:"));
    assert!(text.contains("imports: [jwt, redis]"));
    assert!(
        text.contains("../config"),
        "dependencies must include ../config; got: {text}"
    );
    assert!(
        text.contains("./types"),
        "dependencies must include ./types; got: {text}"
    );
    assert!(text.contains("loc: 12"));
}
