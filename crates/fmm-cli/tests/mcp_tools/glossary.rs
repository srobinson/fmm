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
