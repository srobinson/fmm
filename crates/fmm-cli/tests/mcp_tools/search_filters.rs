use crate::support::{call_tool_text, setup_mcp_server};
use serde_json::json;

#[test]
fn search_depends_on_filter() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"depends_on": "config"}));

    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("src/db/pool.ts"));
}

#[test]
fn search_depends_on_full_manifest_path() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"depends_on": "src/config.ts"}),
    );

    assert!(
        text.contains("src/auth/session.ts"),
        "session.ts should appear; got: {text}"
    );
    assert!(
        text.contains("src/db/pool.ts"),
        "pool.ts should appear; got: {text}"
    );

    let result_lines: Vec<&str> = text.lines().filter(|line| !line.starts_with('#')).collect();
    assert!(
        !result_lines
            .iter()
            .any(|line| line.trim_start().starts_with("src/config.ts")),
        "config.ts should not appear as a dependent of itself; got: {text}"
    );
}

#[test]
fn search_loc_range() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"min_loc": 7, "max_loc": 10}));

    assert!(text.contains("src/config.ts"));
    assert!(text.contains("src/db/pool.ts"));
    assert!(text.contains("src/utils/crypto.ts"));
    assert!(!text.contains("src/auth/session.ts"));
    assert!(!text.contains("src/auth/types.ts"));
}

#[test]
fn search_imports_filter() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"imports": "jwt"}));

    assert!(text.contains("src/auth/session.ts"));
}
