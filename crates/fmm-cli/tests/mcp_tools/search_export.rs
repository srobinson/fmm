use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn search_export_fuzzy_fallback() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"export": "Password"}));

    assert!(text.contains("crypto"));
}

#[test]
fn search_export_exact_still_works() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"export": "createSession"}));

    assert!(text.contains("src/auth/session.ts"));
}

#[test]
fn search_export_fuzzy_case_insensitive() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"export": "pool"}));

    assert!(text.contains("pool"));
}

#[test]
fn search_results_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"export": "createSession"}));

    assert!(text.contains("[6, 8]"));
}

#[test]
fn search_export_limit_caps_results() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"export": "create", "limit": 1}),
    );
    let rows: Vec<&str> = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect();

    assert_eq!(rows.len(), 1, "got: {text}");
    assert!(text.contains("# showing: 1-1 of 2"), "got: {text}");
}
