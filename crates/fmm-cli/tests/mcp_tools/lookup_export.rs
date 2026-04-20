use crate::support::{
    call_tool_expect_error, call_tool_text, setup_collision_server, setup_mcp_server,
};
use serde_json::json;

#[test]
fn lookup_export_returns_sidecar_yaml() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "createSession"}),
    );

    assert!(text.starts_with("---"));
    assert!(text.contains("symbol: createSession"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("lines: [6, 8]"));
    assert!(text.contains("exports:"));
    assert!(text.contains("imports: [jwt, redis]"));
    assert!(text.contains("loc: 12"));
}

#[test]
fn lookup_export_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(&server, "fmm_lookup_export", json!({"name": "nonExistent"}));
    assert!(text.contains("not found"));
}

#[test]
fn lookup_export_collision_emits_disclosure_note() {
    let (_tmp, server) = setup_collision_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "DispatchConfig"}),
    );

    assert!(
        text.contains("symbol: DispatchConfig"),
        "primary symbol missing:\n{}",
        text
    );
    assert!(
        text.contains("1 additional definition(s) found"),
        "collision disclosure missing:\n{}",
        text
    );
    assert!(
        text.contains("fmm_glossary"),
        "fmm_glossary reference missing from disclosure:\n{}",
        text
    );
}

#[test]
fn lookup_export_no_collision_no_disclosure() {
    let (_tmp, server) = setup_collision_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "createSession"}),
    );

    assert!(
        text.contains("symbol: createSession"),
        "symbol missing:\n{}",
        text
    );
    assert!(
        !text.contains("additional definition"),
        "unexpected collision note for unique export:\n{}",
        text
    );
}
