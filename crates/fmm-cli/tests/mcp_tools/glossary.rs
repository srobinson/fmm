use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn glossary_dotted_no_match_uses_standard_empty_output() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Missing.symbol"}),
    );

    assert!(text.contains("(no matching exports)"), "got:\n{text}");
    assert_ne!(text.trim(), "---");
}

#[test]
fn glossary_exact_excludes_substring_matches() {
    let (_tmp, server) = setup_mcp_server();
    let fuzzy = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Config", "mode": "all"}),
    );
    assert!(fuzzy.contains("AppConfig"), "got:\n{fuzzy}");
    assert!(fuzzy.contains("loadConfig"), "got:\n{fuzzy}");

    let exact = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Config", "mode": "all", "exact": true}),
    );
    assert!(exact.contains("(no matching exports)"), "got:\n{exact}");
    assert!(!exact.contains("AppConfig"), "got:\n{exact}");
    assert!(!exact.contains("loadConfig"), "got:\n{exact}");
}
