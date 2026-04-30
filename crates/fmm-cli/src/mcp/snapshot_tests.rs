//! Insta snapshot tests for MCP tool response formats.
//!
//! Each test constructs a known manifest, calls the tool via `McpServer::call_tool`,
//! and snapshots the text response. This catches unintended format changes that break
//! agent behavior.

use super::McpServer;
use fmm_core::store::FmmStore;
use fmm_core::types::PreserializedRow;
use fmm_store::InMemoryStore;
use std::collections::HashMap;

/// Extract the text content from an MCP tool response JSON.
fn extract_text(response: &serde_json::Value) -> &str {
    response["content"][0]["text"].as_str().unwrap_or("")
}

/// Build an InMemoryStore with a representative set of files for snapshot tests.
fn snapshot_store() -> InMemoryStore {
    let store = InMemoryStore::new();

    let rows = vec![
        make_row(
            "src/app.ts",
            30,
            vec![("createApp", 1, 15), ("AppConfig", 17, 25)],
            vec![],
            vec!["./config".to_string(), "./utils".to_string()],
            vec!["react".to_string()],
        ),
        make_row(
            "src/config.ts",
            20,
            vec![("Config", 1, 18)],
            vec![],
            vec![],
            vec![],
        ),
        make_row(
            "src/utils.ts",
            45,
            vec![
                ("formatDate", 1, 10),
                ("parseJSON", 12, 25),
                ("debounce", 27, 40),
            ],
            vec![],
            vec![],
            vec!["lodash".to_string()],
        ),
        make_row(
            "src/server.ts",
            60,
            vec![("Server", 1, 55)],
            vec![
                ("Server.start", 10, 30, None),
                ("Server.stop", 32, 50, None),
            ],
            vec!["./config".to_string(), "./utils".to_string()],
            vec!["express".to_string()],
        ),
        make_row(
            "src/test/app.test.ts",
            15,
            vec![],
            vec![],
            vec!["../app".to_string()],
            vec!["vitest".to_string()],
        ),
    ];

    store.write_indexed_files(&rows, true).unwrap();

    // Set up workspace packages
    let mut pkgs = HashMap::new();
    pkgs.insert(
        "core".to_string(),
        std::path::PathBuf::from("/project/packages/core"),
    );
    store.upsert_workspace_packages(&pkgs).unwrap();

    store
}

/// Helper to build a PreserializedRow from readable parameters.
fn make_row(
    path: &str,
    loc: i64,
    exports: Vec<(&str, i64, i64)>,
    methods: Vec<(&str, i64, i64, Option<&str>)>,
    deps: Vec<String>,
    imports: Vec<String>,
) -> PreserializedRow {
    use fmm_core::types::{ExportRecord, MethodRecord};

    let named_imports: HashMap<String, Vec<String>> = HashMap::new();
    let namespace_imports: Vec<String> = Vec::new();
    let function_names: Vec<String> = Vec::new();

    PreserializedRow {
        rel_path: path.to_string(),
        loc,
        mtime: Some("2026-03-18T00:00:00+00:00".to_string()),
        imports_json: serde_json::to_string(&imports).unwrap(),
        deps_json: serde_json::to_string(&deps).unwrap(),
        named_imports_json: serde_json::to_string(&named_imports).unwrap(),
        namespace_imports_json: serde_json::to_string(&namespace_imports).unwrap(),
        function_names_json: serde_json::to_string(&function_names).unwrap(),
        indexed_at: "2026-03-18T00:00:00+00:00".to_string(),
        fingerprint: None,
        exports: exports
            .into_iter()
            .map(|(name, start, end)| ExportRecord {
                name: name.to_string(),
                start_line: start,
                end_line: end,
            })
            .collect(),
        methods: methods
            .into_iter()
            .map(|(name, start, end, kind)| MethodRecord {
                dotted_name: name.to_string(),
                start_line: start,
                end_line: end,
                kind: kind.map(String::from),
            })
            .collect(),
    }
}

/// Build an McpServer backed by InMemoryStore with known test data.
fn snapshot_server() -> McpServer<InMemoryStore> {
    let store = snapshot_store();
    let root = std::path::PathBuf::from("/project");
    McpServer::from_store(store, root)
}

// --- Snapshot tests for each tool ---

#[test]
fn snapshot_fmm_list_files() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_list_files", serde_json::json!({}))
        .unwrap();
    insta::assert_snapshot!("fmm_list_files", extract_text(&response));
}

#[test]
fn snapshot_fmm_list_files_with_directory() {
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"directory": "src/test"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_list_files_directory", extract_text(&response));
}

#[test]
fn snapshot_fmm_list_files_grouped() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_list_files", serde_json::json!({"group_by": "subdir"}))
        .unwrap();
    insta::assert_snapshot!("fmm_list_files_grouped", extract_text(&response));
}

#[test]
fn snapshot_fmm_lookup_export() {
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_lookup_export",
            serde_json::json!({"name": "createApp"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_lookup_export", extract_text(&response));
}

#[test]
fn snapshot_fmm_lookup_export_not_found() {
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_lookup_export",
            serde_json::json!({"name": "NonExistent"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_lookup_export_not_found", extract_text(&response));
}

#[test]
fn snapshot_fmm_list_exports_file() {
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_list_exports",
            serde_json::json!({"file": "src/utils.ts"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_list_exports_file", extract_text(&response));
}

#[test]
fn snapshot_fmm_list_exports_all() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_list_exports", serde_json::json!({}))
        .unwrap();
    insta::assert_snapshot!("fmm_list_exports_all", extract_text(&response));
}

#[test]
fn snapshot_fmm_list_exports_pattern() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_list_exports", serde_json::json!({"pattern": "app"}))
        .unwrap();
    insta::assert_snapshot!("fmm_list_exports_pattern", extract_text(&response));
}

#[test]
fn snapshot_fmm_search() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_search", serde_json::json!({"query": "config"}))
        .unwrap();
    insta::assert_snapshot!("fmm_search", extract_text(&response));
}

#[test]
fn snapshot_fmm_glossary() {
    let server = snapshot_server();
    let response = server
        .call_tool("fmm_glossary", serde_json::json!({"pattern": "Server"}))
        .unwrap();
    insta::assert_snapshot!("fmm_glossary", extract_text(&response));
}

#[test]
fn snapshot_fmm_dependency_graph() {
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_dependency_graph",
            serde_json::json!({"file": "src/app.ts"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_dependency_graph", extract_text(&response));
}

#[test]
fn snapshot_fmm_file_outline() {
    // file_outline needs an actual file for on-demand tree-sitter parse,
    // but the index-based path (without include_private) uses manifest data.
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_file_outline",
            serde_json::json!({"file": "src/server.ts"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_file_outline", extract_text(&response));
}

#[test]
fn snapshot_fmm_read_symbol_not_found() {
    // read_symbol with a missing symbol produces a deterministic error format
    let server = snapshot_server();
    let response = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "MissingSymbol"}),
        )
        .unwrap();
    insta::assert_snapshot!("fmm_read_symbol_not_found", extract_text(&response));
}

#[test]
fn snapshot_fmm_read_symbol_with_source() {
    // read_symbol with an actual file on disk
    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
        src_dir.join("hello.ts"),
        "export function greet(name: string): string {\n  return `Hello, ${name}!`;\n}\n",
    )
    .unwrap();

    let store = InMemoryStore::new();
    let row = make_row(
        "src/hello.ts",
        3,
        vec![("greet", 1, 3)],
        vec![],
        vec![],
        vec![],
    );
    store.write_indexed_files(&[row], true).unwrap();

    let server = McpServer::from_store(store, dir.path().to_path_buf());
    let response = server
        .call_tool("fmm_read_symbol", serde_json::json!({"name": "greet"}))
        .unwrap();
    insta::assert_snapshot!("fmm_read_symbol", extract_text(&response));
}
