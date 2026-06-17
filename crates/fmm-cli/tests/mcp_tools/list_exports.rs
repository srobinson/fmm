use crate::support::{call_tool_text, setup_mcp_server, write_file};
use serde_json::json;

fn setup_export_filter_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write_file(
        root,
        "src/helpers.ts",
        "export function loadHelper() {\n  return {};\n}\n",
    );
    write_file(
        root,
        "src/helpers.test.ts",
        "export function testHelper() {\n  return {};\n}\n",
    );
    write_file(
        root,
        "src/inline.rs",
        "pub fn source_api() {}\n\n#[cfg(test)]\nmod integration_support {\n    pub fn wal_mode_is_active() {}\n}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn list_exports_by_file() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.starts_with("---"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("exports:"));
    assert!(text.contains("createSession: [6, 8]"));
    assert!(text.contains("validateSession: [10, 12]"));
}

#[test]
fn list_exports_by_pattern() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_list_exports", json!({"pattern": "session"}));

    assert!(text.contains("createSession"));
    assert!(text.contains("validateSession"));
    assert!(text.contains("SessionToken"));
    assert!(text.contains("src/auth/session.ts"));
}

#[test]
fn list_exports_all() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_list_exports", json!({}));

    assert!(text.contains("---"));
    assert!(text.contains("file:"));
    assert!(text.contains("exports:"));
}

#[test]
fn list_exports_directory_filter_pattern() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "session", "directory": "src/auth/"}),
    );
    assert!(
        text.contains("createSession"),
        "createSession should appear; got: {text}"
    );
    assert!(
        !text.contains("Pool"),
        "Pool from src/db/ should not appear with directory=src/auth/; got: {text}"
    );
}

#[test]
fn list_exports_directory_filter_all() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_list_exports", json!({"directory": "src/db/"}));
    assert!(
        text.contains("Pool"),
        "Pool should appear under src/db/; got: {text}"
    );
    assert!(
        !text.contains("createSession"),
        "createSession from src/auth/ should not appear; got: {text}"
    );
}

#[test]
fn list_exports_filter_source_excludes_test_exports() {
    let (_tmp, server) = setup_export_filter_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "Helper", "filter": "source"}),
    );

    assert!(text.contains("loadHelper"), "got: {text}");
    assert!(!text.contains("testHelper"), "got: {text}");
}

#[test]
fn list_exports_filter_tests_includes_only_test_exports() {
    let (_tmp, server) = setup_export_filter_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "Helper", "filter": "tests"}),
    );

    assert!(text.contains("testHelper"), "got: {text}");
    assert!(!text.contains("loadHelper"), "got: {text}");
}

#[test]
fn list_exports_filter_source_excludes_inline_test_module_exports() {
    let (_tmp, server) = setup_export_filter_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "wal_mode_is_active", "filter": "source"}),
    );

    assert!(!text.contains("wal_mode_is_active"), "got: {text}");
}

#[test]
fn list_exports_filter_tests_includes_inline_test_module_exports() {
    let (_tmp, server) = setup_export_filter_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "wal_mode_is_active", "filter": "tests"}),
    );

    assert!(text.contains("wal_mode_is_active"), "got: {text}");
    assert!(text.contains("src/inline.rs"), "got: {text}");
}

#[test]
fn list_exports_filter_source_excludes_inline_test_module_exports_in_all_mode() {
    let (_tmp, server) = setup_export_filter_server();
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"directory": "src/", "filter": "source"}),
    );

    assert!(text.contains("source_api"), "got: {text}");
    assert!(!text.contains("wal_mode_is_active"), "got: {text}");
}

#[test]
fn list_exports_pattern_pagination_limit_and_offset() {
    let (_tmp, server) = setup_mcp_server();

    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "a", "limit": 2, "offset": 0}),
    );
    assert!(
        text.contains("showing: 1-2 of"),
        "should show pagination header; got: {text}"
    );
    assert!(
        text.contains("offset=2"),
        "should hint next offset=2; got: {text}"
    );

    let text2 = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "a", "limit": 2, "offset": 2}),
    );
    assert!(
        text2.contains("showing: 3-4 of"),
        "second page header; got: {text2}"
    );

    let text3 = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "a", "limit": 10, "offset": 0}),
    );
    assert!(
        !text3.contains("showing:"),
        "no pagination header when all results fit; got: {text3}"
    );
}

#[test]
fn list_exports_all_pagination_limit_and_offset() {
    let (_tmp, server) = setup_mcp_server();

    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"limit": 2, "offset": 0}),
    );
    assert!(
        text.contains("showing: 1-2 of 5"),
        "all-mode page 1 header; got: {text}"
    );
    assert!(text.contains("offset=2"), "should hint next=2; got: {text}");

    let text2 = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"limit": 2, "offset": 4}),
    );
    assert!(
        text2.contains("showing: 5-5 of 5"),
        "all-mode last page header; got: {text2}"
    );
    assert!(
        !text2.contains("offset=6"),
        "no next hint on last page; got: {text2}"
    );

    let text3 = call_tool_text(&server, "fmm_list_exports", json!({"limit": 200}));
    assert!(
        !text3.contains("showing:"),
        "no header when all fit; got: {text3}"
    );
}
