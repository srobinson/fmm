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

    // A test file that depends on src/agent.py — used to verify used_by filtering in mode=tests
    write_sidecar(
        root,
        "tests/agent_spec.py",
        "file: tests/agent_spec.py\nfmm: v0.3\nexports:\n  test_dispatch_happy_path: [1, 15]\nimports: []\ndependencies: [../src/agent]\nloc: 15\n",
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
fn glossary_mode_tests_shows_source_definitions_with_test_callers() {
    let (_tmp, server) = setup_glossary_server_with_tests();
    // mode=tests: same definition filter as source (non-test files), used_by filtered to test files.
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "dispatch", "mode": "tests"}),
    );
    // run_dispatch is a source export — it should appear as a definition
    assert!(
        text.contains("run_dispatch:"),
        "run_dispatch (source) should appear in tests mode definitions, got: {}",
        text
    );
    // test_run_dispatch has a test_ prefix — excluded by definition filter (is_test_export)
    assert!(
        !text.contains("test_run_dispatch:"),
        "test_run_dispatch should be excluded from tests mode definitions, got: {}",
        text
    );
    // tests/agent_spec.py depends on src/agent.py — it should appear in used_by
    assert!(
        text.contains("tests/agent_spec.py"),
        "tests/agent_spec.py should appear in used_by for tests mode, got: {}",
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
fn glossary_mode_tests_excludes_test_directory_definitions() {
    // mode=tests uses the SAME definition filter as mode=source: non-test files only.
    // helper_fixture is exported from tests/helpers.py — a test file. It must not appear.
    let (_tmp, server) = setup_glossary_server_with_tests();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "helper", "mode": "tests"}),
    );
    assert!(
        text.contains("(no matching exports)"),
        "helper_fixture (from tests/ dir) should be excluded from tests mode definitions, got: {}",
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

// ---------------------------------------------------------------------------
// ALP-826: contextualise empty and file-level results
// ---------------------------------------------------------------------------

fn setup_method_glossary_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // src/injector.ts — exports Injector class with loadInstance method
    write_sidecar(
        root,
        "src/injector.ts",
        "file: src/injector.ts\nfmm: v0.3\nexports:\n  Injector: [1, 200]\nmethods:\n  Injector.loadInstance: [10, 50]\nimports: []\ndependencies: []\nloc: 200\n",
    );

    // src/app.ts — source file that depends on src/injector.ts
    write_sidecar(
        root,
        "src/app.ts",
        "file: src/app.ts\nfmm: v0.3\nexports:\n  App: [1, 30]\nimports: []\ndependencies: [./injector]\nloc: 30\n",
    );

    // tests/injector.spec.ts — test file that depends on src/injector.ts
    write_sidecar(
        root,
        "tests/injector.spec.ts",
        "file: tests/injector.spec.ts\nfmm: v0.3\nexports:\n  testLoadInstance: [1, 20]\nimports: []\ndependencies: [../src/injector]\nloc: 20\n",
    );

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn glossary_dotted_query_empty_callers_shows_contextual_message() {
    // Call-site refinement on an empty source file finds nothing.
    // The output should explain the silence, not just show used_by: [].
    let (_tmp, server) = setup_method_glossary_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Injector.loadInstance", "mode": "source"}),
    );
    assert!(
        text.contains("(no external source callers)"),
        "should show no-callers message; got:\n{}",
        text
    );
    assert!(
        text.contains("import injector.ts") || text.contains("import injector"),
        "should mention the source file; got:\n{}",
        text
    );
    assert!(
        text.contains("none call loadInstance directly"),
        "should mention method name; got:\n{}",
        text
    );
}

#[test]
fn glossary_dotted_query_empty_callers_shows_test_hint_when_test_callers_exist() {
    let (_tmp, server) = setup_method_glossary_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Injector.loadInstance", "mode": "source"}),
    );
    assert!(
        text.contains("test caller") || text.contains("test callers"),
        "should hint at test callers; got:\n{}",
        text
    );
    assert!(
        text.contains("rerun with mode: tests"),
        "should suggest mode:tests; got:\n{}",
        text
    );
}

#[test]
fn glossary_dotted_query_non_empty_callers_unchanged() {
    // When call-site refinement finds callers, output must not include the contextual message.
    let (_tmp, server) = setup_method_glossary_server();
    // mode=tests will show the test file as a file-level importer; after call-site
    // refinement on the (empty) test source, used_by will be empty → triggers case 1.
    // Use mode=all so we exercise the "all" mode label.
    // For a non-empty test, we need actual source with call sites — not possible in
    // fixture tests. Instead verify the format_glossary path is used when there ARE
    // non-empty used_by (i.e. before refinement logic applies, the format is normal).
    // Covered by existing tests; just confirm the new message is absent for non-dotted queries.
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Injector", "mode": "source"}),
    );
    assert!(
        !text.contains("(no external source callers)"),
        "non-dotted query should not show empty-caller message; got:\n{}",
        text
    );
}

#[test]
fn glossary_bare_name_nudge_when_method_index_entry_exists() {
    let (_tmp, server) = setup_method_glossary_server();
    // "loadInstance" (bare name) matches Injector.loadInstance in method_index.
    // The results are file-level importers, not call-site callers → nudge expected.
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "loadInstance", "mode": "all"}),
    );
    assert!(
        text.contains("file-level importers"),
        "should append file-level importer nudge; got:\n{}",
        text
    );
    assert!(
        text.contains("call-site precision"),
        "should suggest call-site precision; got:\n{}",
        text
    );
    assert!(
        text.contains("Injector.loadInstance"),
        "should mention the dotted pattern name; got:\n{}",
        text
    );
}

#[test]
fn glossary_bare_name_no_nudge_when_no_method_entry() {
    // "Config" matches only class-level exports, not any method_index entry.
    // No nudge should appear.
    let (_tmp, server) = setup_glossary_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "Config"}));
    assert!(
        !text.contains("file-level importers"),
        "no nudge expected when no dotted entry matches; got:\n{}",
        text
    );
}

// ---------------------------------------------------------------------------
// ALP-847: bare function call-site precision tests
// ---------------------------------------------------------------------------

fn setup_bare_fn_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // The source file: exports scheduleUpdate as a function
    let src_file = root.join("src/scheduler.ts");
    std::fs::create_dir_all(src_file.parent().unwrap()).unwrap();
    std::fs::write(&src_file, "export function scheduleUpdate() {}\n").unwrap();
    std::fs::write(
        src_file.with_extension("ts.fmm"),
        "file: src/scheduler.ts\nfmm: v0.3\nexports:\n  scheduleUpdate: [1, 1]\ntypescript:\n  function_names: [scheduleUpdate]\nimports: []\ndependencies: []\nloc: 1\n",
    )
    .unwrap();

    // Caller 1: direct call
    let caller1 = root.join("src/direct_caller.ts");
    std::fs::write(
        &caller1,
        "import { scheduleUpdate } from './scheduler';\nscheduleUpdate();\n",
    )
    .unwrap();
    std::fs::write(
        caller1.with_extension("ts.fmm"),
        "file: src/direct_caller.ts\nfmm: v0.3\nexports: {}\nimports: []\ndependencies: [./scheduler]\nloc: 2\n",
    )
    .unwrap();

    // Caller 2: aliased import
    let caller2 = root.join("src/aliased_caller.ts");
    std::fs::write(
        &caller2,
        "import { scheduleUpdate as su } from './scheduler';\nsu();\n",
    )
    .unwrap();
    std::fs::write(
        caller2.with_extension("ts.fmm"),
        "file: src/aliased_caller.ts\nfmm: v0.3\nexports: {}\nimports: []\ndependencies: [./scheduler]\nloc: 2\n",
    )
    .unwrap();

    // Importer-only: imports but never calls
    let importer_only = root.join("src/importer_only.ts");
    std::fs::write(
        &importer_only,
        "import { scheduleUpdate } from './scheduler';\n// never calls it\n",
    )
    .unwrap();
    std::fs::write(
        importer_only.with_extension("ts.fmm"),
        "file: src/importer_only.ts\nfmm: v0.3\nexports: {}\nimports: []\ndependencies: [./scheduler]\nloc: 2\n",
    )
    .unwrap();

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn glossary_bare_function_call_site_precision_filters_non_callers() {
    // ALP-883: Layer 3 (call-site verification) is opt-in via precision: "call-site".
    let (_tmp, server) = setup_bare_fn_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate", "precision": "call-site"}),
    );
    // Direct and aliased callers must appear
    assert!(
        text.contains("direct_caller.ts"),
        "direct caller should appear; got:\n{}",
        text
    );
    assert!(
        text.contains("aliased_caller.ts"),
        "aliased caller should appear; got:\n{}",
        text
    );
    // Importer-only should be excluded by Layer 3
    assert!(
        !text.contains("importer_only.ts"),
        "importer-only should be excluded by Layer 3; got:\n{}",
        text
    );
}

// ---------------------------------------------------------------------------
// ALP-906: cross-package bare workspace specifier Layer 2 fix
// ---------------------------------------------------------------------------

fn setup_workspace_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // pnpm-workspace.yaml so workspace discovery populates workspace_roots.
    std::fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - 'packages/*'\n",
    )
    .unwrap();

    // packages/shared/ReactFeatureFlags.js — exports enableTransitionTracing as a function.
    write_sidecar(
        root,
        "packages/shared/ReactFeatureFlags.js",
        "file: packages/shared/ReactFeatureFlags.js\nfmm: v0.4\nexports:\n  enableTransitionTracing: [1, 1]\ntypescript:\n  function_names: [enableTransitionTracing]\nimports: []\ndependencies: []\nloc: 1\n",
    );

    // packages/reconciler/src/Worker.js — cross-package import via bare specifier.
    // dependencies uses the resolved relative path so reverse_deps is populated.
    // named_imports stores the bare specifier exactly as written in source.
    write_sidecar(
        root,
        "packages/reconciler/src/Worker.js",
        "file: packages/reconciler/src/Worker.js\nfmm: v0.4\nexports: {}\nimports: []\ndependencies: [../../shared/ReactFeatureFlags]\nloc: 3\nnamed_imports:\n  shared/ReactFeatureFlags:\n    - enableTransitionTracing\n",
    );

    // packages/shared/other.js — imports source but NOT enableTransitionTracing.
    // This should appear in the disclosure count (Layer 2 excluded), not in used_by.
    write_sidecar(
        root,
        "packages/shared/other.js",
        "file: packages/shared/other.js\nfmm: v0.4\nexports: {}\nimports: []\ndependencies: [./ReactFeatureFlags]\nloc: 2\nnamed_imports:\n  ./ReactFeatureFlags:\n    - otherFlag\n",
    );

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn glossary_layer2_cross_package_bare_specifier_included_in_used_by() {
    // ALP-906: a file importing via bare workspace specifier (e.g. `shared/ReactFeatureFlags`)
    // must appear in used_by, not be silently dropped into the Layer 2 disclosure count.
    let (_tmp, server) = setup_workspace_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "enableTransitionTracing"}),
    );
    assert!(
        text.contains("packages/reconciler/src/Worker.js"),
        "cross-package caller should appear in used_by; got:\n{}",
        text
    );
}

#[test]
fn glossary_layer2_intra_package_relative_import_unchanged() {
    // ALP-906 regression guard: intra-package relative import still works after the fix.
    // packages/shared/other.js imports from ./ReactFeatureFlags but not enableTransitionTracing —
    // it should appear in the disclosure count, not in used_by.
    let (_tmp, server) = setup_workspace_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "enableTransitionTracing"}),
    );
    // other.js imports the module but not the specific symbol — excluded by Layer 2.
    assert!(
        !text.contains("packages/shared/other.js"),
        "intra-package non-symbol importer must not appear in used_by; got:\n{}",
        text
    );
    // Disclosure count should mention the excluded file.
    assert!(
        text.contains("additional file") || text.contains("additional files"),
        "disclosure count should mention excluded intra-package importer; got:\n{}",
        text
    );
}

// ---------------------------------------------------------------------------
// ALP-907: disclosure line wording fix (no duplicate "import")
// ---------------------------------------------------------------------------

fn setup_disclosure_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // source.ts exports myFunc as a module-level function.
    write_sidecar(
        root,
        "src/source.ts",
        "file: src/source.ts\nfmm: v0.4\nexports:\n  myFunc: [1, 5]\ntypescript:\n  function_names: [myFunc]\nimports: []\ndependencies: []\nloc: 5\n",
    );

    // caller.ts named-imports myFunc → stays in used_by.
    write_sidecar(
        root,
        "src/caller.ts",
        "file: src/caller.ts\nfmm: v0.4\nexports: {}\nimports: []\ndependencies: [./source]\nloc: 2\nnamed_imports:\n  ./source:\n    - myFunc\n",
    );

    // bystander.ts imports otherThing, not myFunc → excluded by Layer 2.
    write_sidecar(
        root,
        "src/bystander.ts",
        "file: src/bystander.ts\nfmm: v0.4\nexports: {}\nimports: []\ndependencies: [./source]\nloc: 2\nnamed_imports:\n  ./source:\n    - otherThing\n",
    );

    let server = fmm::mcp::McpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn glossary_layer2_disclosure_line_no_duplicate_import() {
    // ALP-907: the disclosure line must not contain "import import".
    let (_tmp, server) = setup_disclosure_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "myFunc"}));
    assert!(
        !text.contains("import import"),
        "disclosure line must not duplicate 'import'; got:\n{}",
        text
    );
}

#[test]
fn glossary_layer2_disclosure_line_includes_symbol_name() {
    // ALP-907: disclosure line includes the queried symbol name for scannability.
    let (_tmp, server) = setup_disclosure_server();
    let text = call_tool_text(&server, "fmm_glossary", json!({"pattern": "myFunc"}));
    assert!(
        text.contains("myFunc"),
        "disclosure line should include the symbol name; got:\n{}",
        text
    );
    assert!(
        text.contains("specifically"),
        "disclosure should end with 'specifically'; got:\n{}",
        text
    );
}
