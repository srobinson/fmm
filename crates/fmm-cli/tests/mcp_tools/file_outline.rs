use crate::support::{call_tool_expect_error, call_tool_text, setup_mcp_server, write_file};
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
    assert!(text.contains("createSession:\n    lines: [6, 8]\n    size: 3"));
    assert!(text.contains("validateSession:\n    lines: [10, 12]\n    size: 3"));
    assert!(text.contains("signature: export function createSession"));
    assert!(text.contains("visibility: public"));
    assert!(text.contains("kind: fn"));
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
    assert!(text.contains("File not found in workspace: src/nonexistent.ts"));
    assert!(!text.contains("Run 'fmm generate'"), "got: {text}");
}

#[test]
fn file_outline_exists_but_missing_from_index() {
    let (tmp, server) = setup_mcp_server();
    write_file(
        tmp.path(),
        "src/new.ts",
        "export function createNew() {\n  return {};\n}\n",
    );
    let text = call_tool_expect_error(&server, "fmm_file_outline", json!({"file": "src/new.ts"}));
    assert!(text.contains("File exists but is missing from the fmm index: src/new.ts"));
    assert!(text.contains("Run 'fmm generate'."), "got: {text}");
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

#[test]
fn file_outline_truncate_false_bypasses_response_cap() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let mut source = String::new();
    for i in 0..500 {
        source.push_str(&format!(
            "export function function{i:03}() {{\n  return {i};\n}}\n\n"
        ));
    }
    write_file(root, "src/large.ts", &source);
    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());

    let default_text = call_tool_text(&server, "fmm_file_outline", json!({"file": "src/large.ts"}));
    assert!(
        default_text.contains("[Truncated"),
        "default response should be capped; got {} bytes",
        default_text.len()
    );
    assert!(
        default_text.contains("truncate: false to get the full response"),
        "truncation hint must reference a real fmm_file_outline parameter; got: {default_text}"
    );

    let full_text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/large.ts", "truncate": false}),
    );
    assert!(
        !full_text.contains("[Truncated"),
        "truncate=false must leave outline uncapped"
    );
    assert!(full_text.contains("function000:"));
    assert!(full_text.contains("function499:"));
}
