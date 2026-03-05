//! Integration tests for the fmm_glossary MCP tool.
//!
//! Tests setup temp dirs with sidecars (like mcp_tools.rs) and call through
//! McpServer::call_tool to test the real JSON-RPC path.

use serde_json::json;

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn write_sidecar(dir: &std::path::Path, rel_path: &str, content: &str) {
    let full = dir.join(rel_path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, "").unwrap(); // source placeholder
    let sidecar = format!("{}.fmm", full.display());
    std::fs::write(sidecar, content).unwrap();
}

fn setup_glossary_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // src/config/types.ts — exports Config [1-5]
    write_sidecar(
        root,
        "src/config/types.ts",
        "file: src/config/types.ts\nfmm: v0.3\nexports:\n  Config: [1, 5]\nimports: []\ndependencies: []\nloc: 10\n",
    );

    // src/config/defaults.ts — also exports Config [3-8] (duplicate)
    write_sidecar(
        root,
        "src/config/defaults.ts",
        "file: src/config/defaults.ts\nfmm: v0.3\nexports:\n  Config: [3, 8]\nimports: []\ndependencies: []\nloc: 15\n",
    );

    // src/app.ts — imports from config/types and config/defaults
    write_sidecar(
        root,
        "src/app.ts",
        "file: src/app.ts\nfmm: v0.3\nexports:\n  App: [1, 10]\nimports: []\ndependencies: [./config/types, ./config/defaults]\nloc: 30\n",
    );

    // src/server.ts — imports only from config/types
    write_sidecar(
        root,
        "src/server.ts",
        "file: src/server.ts\nfmm: v0.3\nexports:\n  Server: [1, 20]\nimports: []\ndependencies: [./config/types]\nloc: 50\n",
    );

    // src/utils.ts — exports something unrelated
    write_sidecar(
        root,
        "src/utils.ts",
        "file: src/utils.ts\nfmm: v0.3\nexports:\n  formatDate: [1, 5]\nimports: []\ndependencies: []\nloc: 8\n",
    );

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

fn setup_glossary_server_with_tests() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Normal Python source file with a real export and a test function
    write_sidecar(
        root,
        "src/agent.py",
        "file: src/agent.py\nfmm: v0.3\nexports:\n  run_dispatch: [1, 50]\n  test_run_dispatch: [52, 80]\nimports: []\ndependencies: []\nloc: 80\n",
    );

    // Go test file (TestRunDispatch is a Go test)
    write_sidecar(
        root,
        "agent_test.go",
        "file: agent_test.go\nfmm: v0.3\nexports:\n  TestRunDispatch: [1, 20]\nimports: []\ndependencies: []\nloc: 20\n",
    );

    // tests/ directory export
    write_sidecar(
        root,
        "tests/helpers.py",
        "file: tests/helpers.py\nfmm: v0.3\nexports:\n  helper_fixture: [1, 10]\nimports: []\ndependencies: []\nloc: 10\n",
    );

    // __tests__/ directory export (JS)
    write_sidecar(
        root,
        "__tests__/utils.ts",
        "file: __tests__/utils.ts\nfmm: v0.3\nexports:\n  mockConfig: [1, 8]\nimports: []\ndependencies: []\nloc: 8\n",
    );

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

fn call_tool_text(server: &fmm::mcp::McpServer, tool: &str, args: serde_json::Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn glossary_empty_pattern_returns_error() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": ""}));
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
    assert!(
        text.contains("pattern is required"),
        "should mention pattern required, got: {}",
        text
    );
}

#[test]
fn glossary_missing_pattern_returns_error() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({}));
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
}

#[test]
fn glossary_exact_symbol_returns_all_definitions() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "Config"}));
    assert!(
        text.contains("Config:"),
        "should have Config entry, got: {}",
        text
    );
    // Both definition files should appear
    assert!(
        text.contains("src/config/types.ts"),
        "should list types.ts definition, got: {}",
        text
    );
    assert!(
        text.contains("src/config/defaults.ts"),
        "should list defaults.ts definition, got: {}",
        text
    );
}

#[test]
fn glossary_used_by_populated_via_dependencies() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "Config"}));
    // src/app.ts depends on both config files
    assert!(
        text.contains("src/app.ts"),
        "src/app.ts should appear in used_by, got: {}",
        text
    );
    // src/server.ts depends on config/types only
    assert!(
        text.contains("src/server.ts"),
        "src/server.ts should appear in used_by, got: {}",
        text
    );
}

#[test]
fn glossary_pattern_filtering_case_insensitive() {
    let (_tmp, server) = setup_glossary_server();
    // "config" (lowercase) should still find "Config"
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "config"}));
    assert!(
        text.contains("Config:"),
        "case-insensitive pattern should match, got: {}",
        text
    );
    // "date" should find formatDate
    let text2 = call_tool_text(&server, "fmm_glossary", json!({"pattern": "date"}));
    assert!(
        text2.contains("formatDate:"),
        "should find formatDate, got: {}",
        text2
    );
    // "config" should not find "formatDate"
    assert!(
        !text.contains("formatDate"),
        "should not match unrelated symbol, got: {}",
        text
    );
}

#[test]
fn glossary_no_match_returns_no_matching_exports() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "zzz_nonexistent_zzz"}),
    );
    assert!(
        text.contains("(no matching exports)"),
        "should report no matches, got: {}",
        text
    );
}

#[test]
fn glossary_limit_respected() {
    let (_tmp, server) = setup_glossary_server();
    // The fixture has exactly two exports containing "a": "App" and "formatDate".
    // With limit=1 we get 1 result and a truncation notice.
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "a", "limit": 1}));
    // Truncation notice must appear: "showing 1/2 matches"
    assert!(
        text.contains("showing 1/2 matches"),
        "should show truncation notice, got: {}",
        text
    );
    // Only one entry rendered (App sorts before formatDate)
    assert!(
        text.contains("App:"),
        "first match should be App (alphabetically first), got: {}",
        text
    );
    assert!(
        !text.contains("formatDate:"),
        "formatDate should be truncated by limit=1, got: {}",
        text
    );
}

#[test]
fn glossary_yaml_format_has_src_and_used_by_keys() {
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "Config"}));
    assert!(
        text.contains("- src:"),
        "should have src: key, got: {}",
        text
    );
    assert!(
        text.contains("used_by:"),
        "should have used_by: key, got: {}",
        text
    );
}

#[test]
fn glossary_mode_source_excludes_test_functions_by_default() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    // Default call — mode not set (defaults to "source")
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "dispatch"}));
    // Real export should appear
    assert!(
        text.contains("run_dispatch:"),
        "run_dispatch should be included, got: {}",
        text
    );
    // test_ prefixed name should be filtered out
    assert!(
        !text.contains("test_run_dispatch"),
        "test_run_dispatch should be excluded in source mode, got: {}",
        text
    );
}

#[test]
fn glossary_mode_all_shows_test_functions() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "dispatch", "mode": "all"}),
    );
    assert!(
        text.contains("run_dispatch:"),
        "run_dispatch should be included, got: {}",
        text
    );
    assert!(
        text.contains("test_run_dispatch:"),
        "test_run_dispatch should be included with mode=all, got: {}",
        text
    );
}

#[test]
fn glossary_mode_tests_returns_only_test_exports() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    // mode=tests: only test symbols returned
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "dispatch", "mode": "tests"}),
    );
    // "run_dispatch:" is a substring of "test_run_dispatch:" so check for the full entry line
    assert!(
        !text.contains("\nrun_dispatch:"),
        "run_dispatch (source) should be excluded in tests mode, got: {}",
        text
    );
    assert!(
        text.contains("test_run_dispatch:"),
        "test_run_dispatch should appear in tests mode, got: {}",
        text
    );
}

#[test]
fn glossary_mode_source_excludes_test_directory_exports_by_default() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    // helper_fixture is in tests/ dir; mockConfig is in __tests__/ dir
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "helper"}));
    assert!(
        text.contains("(no matching exports)"),
        "tests/ exports should be excluded in source mode, got: {}",
        text
    );

    let text2 = call_tool_text(&server, "fmm_glossary", json!({"pattern": "mock"}));
    assert!(
        text2.contains("(no matching exports)"),
        "__tests__/ exports should be excluded in source mode, got: {}",
        text2
    );
}

#[test]
fn glossary_mode_tests_shows_test_directory_exports() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "helper", "mode": "tests"}),
    );
    assert!(
        text.contains("helper_fixture:"),
        "helper_fixture should appear with mode=tests, got: {}",
        text
    );
}

#[test]
fn glossary_mode_source_excludes_go_test_prefix_by_default() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    // TestRunDispatch is in agent_test.go AND has a Test prefix
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "RunDispatch"}));
    assert!(
        text.contains("(no matching exports)"),
        "TestRunDispatch should be excluded in source mode, got: {}",
        text
    );
}

#[test]
fn glossary_default_limit_is_ten() {
    // Build a fixture with 11 distinct exports all matching "item"
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    for i in 1..=11 {
        let filename = format!("src/mod{i}.ts");
        let export = format!("item{i}");
        let content = format!(
            "file: {filename}\nfmm: v0.3\nexports:\n  {export}: [1, 5]\nimports: []\ndependencies: []\nloc: 5\n"
        );
        write_sidecar(root, &filename, &content);
    }
    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "item"}));
    // 11 matches, default limit 10 → truncation notice
    assert!(
        text.contains("showing 10/11 matches"),
        "default limit should be 10, got: {}",
        text
    );
}
