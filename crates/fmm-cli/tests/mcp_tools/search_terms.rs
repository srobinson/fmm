use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn search_term_finds_exact_export() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "createSession"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("[6, 8]"));
}

#[test]
fn search_term_finds_fuzzy_exports() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "session"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("validateSession"));
    assert!(text.contains("SessionToken"));
}

#[test]
fn search_term_finds_file_path_matches() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "crypto"}));

    assert!(text.contains("FILES"));
    assert!(text.contains("crypto"));
}

#[test]
fn search_term_finds_import_matches() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "bcrypt"}));

    assert!(text.contains("IMPORTS"));
    assert!(text.contains("bcrypt"));
    assert!(text.contains("src/utils/crypto.ts"));
}

#[test]
fn search_term_returns_grouped_sections() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "config"}));

    assert!(text.contains("config") || text.contains("Config"));
}

#[test]
fn search_term_case_insensitive() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "POOL"}));

    assert!(text.contains("Pool") || text.contains("createPool"));
    assert!(text.contains("pool"));
}

#[test]
fn search_term_exports_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "hashPassword"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("hashPassword"));
    assert!(text.contains("[3, 5]"));
}
