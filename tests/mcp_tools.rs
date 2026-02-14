//! MCP tool integration tests.
//!
//! Every test calls through McpServer::call_tool — the real JSON-RPC path.
//! A shared fixture builds sidecars in a temp dir so the server loads a
//! realistic manifest with v0.3 line ranges.

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build an MCP server backed by sidecars in a temp dir.
fn setup_mcp_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let auth = src.join("auth");
    let db = src.join("db");
    let utils = src.join("utils");
    std::fs::create_dir_all(&auth).unwrap();
    std::fs::create_dir_all(&db).unwrap();
    std::fs::create_dir_all(&utils).unwrap();

    write_source_and_sidecar(
        &auth.join("session.ts"),
        "import jwt from 'jwt';\nimport redis from 'redis';\nimport { Types } from './types';\nimport { Config } from '../config';\n\nexport function createSession() {\n  return jwt.sign({});\n}\n\nexport function validateSession(token: string) {\n  return jwt.verify(token);\n}\n",
        "file: src/auth/session.ts\nfmm: v0.3\nexports:\n  createSession: [6, 8]\n  validateSession: [10, 12]\nimports: [jwt, redis]\ndependencies: [./types, ../config]\nloc: 12\n",
    );

    write_source_and_sidecar(
        &auth.join("types.ts"),
        "export interface SessionToken {\n  token: string;\n  expires: number;\n}\n\nexport type UserRole = 'admin' | 'user';\n",
        "file: src/auth/types.ts\nfmm: v0.3\nexports:\n  SessionToken: [1, 4]\n  UserRole: [6, 6]\nimports: []\ndependencies: []\nloc: 6\n",
    );

    write_source_and_sidecar(
        &src.join("config.ts"),
        "import dotenv from 'dotenv';\n\nexport function loadConfig() {\n  dotenv.config();\n  return {};\n}\n\nexport interface AppConfig {\n  port: number;\n}\n",
        "file: src/config.ts\nfmm: v0.3\nexports:\n  loadConfig: [3, 6]\n  AppConfig: [8, 10]\nimports: [dotenv]\ndependencies: []\nloc: 10\n",
    );

    write_source_and_sidecar(
        &db.join("pool.ts"),
        "import pg from 'pg';\nimport { Config } from '../config';\n\nexport class Pool {\n  private client: pg.Client;\n}\n\nexport function createPool() {\n  return new Pool();\n}\n",
        "file: src/db/pool.ts\nfmm: v0.3\nexports:\n  Pool: [4, 6]\n  createPool: [8, 10]\nimports: [pg]\ndependencies: [../config]\nloc: 10\n",
    );

    write_source_and_sidecar(
        &utils.join("crypto.ts"),
        "import bcrypt from 'bcrypt';\n\nexport function hashPassword(pw: string) {\n  return bcrypt.hash(pw, 10);\n}\n\nexport function verifyPassword(pw: string, hash: string) {\n  return bcrypt.compare(pw, hash);\n}\n",
        "file: src/utils/crypto.ts\nfmm: v0.3\nexports:\n  hashPassword: [3, 5]\n  verifyPassword: [7, 9]\nimports: [bcrypt]\ndependencies: []\nloc: 9\n",
    );

    let server = fmm::mcp::McpServer::with_root(tmp.path().to_path_buf());
    (tmp, server)
}

fn write_source_and_sidecar(source_path: &std::path::Path, source: &str, sidecar: &str) {
    std::fs::write(source_path, source).unwrap();
    let mut sidecar_path = source_path.as_os_str().to_owned();
    sidecar_path.push(".fmm");
    std::fs::write(std::path::PathBuf::from(sidecar_path), sidecar).unwrap();
}

/// Call a tool and parse the JSON response body.
fn call_tool_json(server: &fmm::mcp::McpServer, tool: &str, args: Value) -> Value {
    let result = server.call_tool(tool, args).unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

/// Call a tool expecting an error response.
fn call_tool_expect_error(server: &fmm::mcp::McpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    assert!(
        result["isError"].as_bool().unwrap_or(false),
        "Expected error but got success"
    );
    result["content"][0]["text"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// Manifest loading (integration — validates sidecar discovery + YAML parse)
// ---------------------------------------------------------------------------

#[test]
fn manifest_loads_from_sidecars() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = fmm::manifest::Manifest::load_from_sidecars(tmp.path()).unwrap();
    assert_eq!(manifest.files.len(), 5);
    assert!(manifest.files.contains_key("src/auth/session.ts"));
    assert!(manifest.files.contains_key("src/auth/types.ts"));
    assert!(manifest.files.contains_key("src/config.ts"));
    assert!(manifest.files.contains_key("src/db/pool.ts"));
    assert!(manifest.files.contains_key("src/utils/crypto.ts"));
}

#[test]
fn export_index_consistency() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = fmm::manifest::Manifest::load_from_sidecars(tmp.path()).unwrap();
    for (export_name, file_path) in &manifest.export_index {
        let entry = manifest.files.get(file_path).unwrap_or_else(|| {
            panic!(
                "Export '{}' points to missing file '{}'",
                export_name, file_path
            )
        });
        assert!(
            entry.exports.contains(export_name),
            "File '{}' doesn't actually export '{}'",
            file_path,
            export_name
        );
    }
}

// ---------------------------------------------------------------------------
// fmm_read_symbol
// ---------------------------------------------------------------------------

#[test]
fn read_symbol_returns_source_lines() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_read_symbol", json!({"name": "createSession"}));

    assert_eq!(parsed["symbol"], "createSession");
    assert_eq!(parsed["file"], "src/auth/session.ts");
    assert_eq!(parsed["lines"], json!([6, 8]));
    let source = parsed["source"].as_str().unwrap();
    assert!(source.contains("createSession"));
    assert!(!source.contains("validateSession"));
}

#[test]
fn read_symbol_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(&server, "fmm_read_symbol", json!({"name": "nonExistent"}));
    assert!(text.contains("not found"));
}

// ---------------------------------------------------------------------------
// fmm_file_outline
// ---------------------------------------------------------------------------

#[test]
fn file_outline_returns_symbols_with_lines() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(
        &server,
        "fmm_file_outline",
        json!({"file": "src/auth/session.ts"}),
    );

    assert_eq!(parsed["file"], "src/auth/session.ts");
    let symbols = parsed["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);

    assert_eq!(symbols[0]["name"], "createSession");
    assert_eq!(symbols[0]["lines"], json!([6, 8]));
    assert_eq!(symbols[0]["size"], 3);

    assert_eq!(symbols[1]["name"], "validateSession");
    assert_eq!(symbols[1]["lines"], json!([10, 12]));
    assert_eq!(symbols[1]["size"], 3);

    assert!(!parsed["imports"].as_array().unwrap().is_empty());
}

#[test]
fn file_outline_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(
        &server,
        "fmm_file_outline",
        json!({"file": "src/nonexistent.ts"}),
    );
    assert!(text.contains("not found"));
}

#[test]
fn file_outline_shows_all_exports() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(
        &server,
        "fmm_file_outline",
        json!({"file": "src/utils/crypto.ts"}),
    );

    let names: Vec<&str> = parsed["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"hashPassword"));
    assert!(names.contains(&"verifyPassword"));
    assert_eq!(parsed["loc"], 9);
}

// ---------------------------------------------------------------------------
// fmm_dependency_graph (previously untested through MCP)
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_upstream_and_downstream() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/auth/session.ts"}),
    );

    assert_eq!(parsed["file"], "src/auth/session.ts");

    let upstream: Vec<&str> = parsed["upstream"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(upstream.contains(&"./types"));
    assert!(upstream.contains(&"../config"));

    let imports: Vec<&str> = parsed["imports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(imports.contains(&"jwt"));
    assert!(imports.contains(&"redis"));
}

#[test]
fn dependency_graph_shows_downstream_dependents() {
    let (_tmp, server) = setup_mcp_server();
    // config.ts is depended on by session.ts and pool.ts
    let parsed = call_tool_json(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts"}),
    );

    let downstream: Vec<&str> = parsed["downstream"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(downstream.contains(&"src/auth/session.ts"));
    assert!(downstream.contains(&"src/db/pool.ts"));
    assert_eq!(downstream.len(), 2);
}

#[test]
fn dependency_graph_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/nonexistent.ts"}),
    );
    assert!(text.contains("not found"));
}

// ---------------------------------------------------------------------------
// fmm_search — universal term search
// ---------------------------------------------------------------------------

#[test]
fn search_term_finds_exact_export() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "createSession"}));

    let exports = parsed["exports"].as_array().unwrap();
    assert!(!exports.is_empty());
    assert_eq!(exports[0]["name"], "createSession");
    assert_eq!(exports[0]["file"], "src/auth/session.ts");
    assert!(exports[0].get("lines").is_some());
}

#[test]
fn search_term_finds_fuzzy_exports() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "session"}));

    let names: Vec<&str> = parsed["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"createSession"));
    assert!(names.contains(&"validateSession"));
    assert!(names.contains(&"SessionToken"));
}

#[test]
fn search_term_finds_file_path_matches() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "crypto"}));

    let files = parsed["files"].as_array().unwrap();
    assert!(!files.is_empty());
    assert!(files
        .iter()
        .any(|f| f["file"].as_str().unwrap().contains("crypto")));
}

#[test]
fn search_term_finds_import_matches() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "bcrypt"}));

    let imports = parsed["imports"].as_array().unwrap();
    assert!(!imports.is_empty());
    assert!(imports
        .iter()
        .any(|i| i["package"].as_str().unwrap() == "bcrypt"));
    let files = imports[0]["files"].as_array().unwrap();
    assert!(!files.is_empty());
}

#[test]
fn search_term_returns_grouped_json() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "config"}));

    assert!(parsed.get("exports").is_some());
    assert!(parsed.get("files").is_some());
    assert!(parsed.get("imports").is_some());
}

#[test]
fn search_term_case_insensitive() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "POOL"}));

    let names: Vec<&str> = parsed["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Pool") || names.contains(&"createPool"));

    let files = parsed["files"].as_array().unwrap();
    assert!(files
        .iter()
        .any(|f| f["file"].as_str().unwrap().contains("pool")));
}

// ---------------------------------------------------------------------------
// fmm_search — export filter (exact + fuzzy fallback)
// ---------------------------------------------------------------------------

#[test]
fn search_export_fuzzy_fallback() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"export": "Password"}));

    let results = parsed.as_array().unwrap();
    assert!(!results.is_empty());
    assert!(results
        .iter()
        .any(|r| r["file"].as_str().unwrap().contains("crypto")));
}

#[test]
fn search_export_exact_still_works() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"export": "createSession"}));

    let results = parsed.as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["file"], "src/auth/session.ts");
}

#[test]
fn search_export_fuzzy_case_insensitive() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"export": "pool"}));

    let results = parsed.as_array().unwrap();
    assert!(!results.is_empty());
    assert!(results
        .iter()
        .any(|r| r["file"].as_str().unwrap().contains("pool")));
}

// ---------------------------------------------------------------------------
// fmm_search — line ranges in output
// ---------------------------------------------------------------------------

#[test]
fn search_results_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"export": "createSession"}));

    let results = parsed.as_array().unwrap();
    assert!(!results.is_empty());
    let exports = results[0]["exports"].as_array().unwrap();
    assert!(
        exports.iter().any(|e| e.get("lines").is_some()),
        "Export results should include line ranges"
    );
}

#[test]
fn search_term_exports_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"term": "hashPassword"}));

    let exports = parsed["exports"].as_array().unwrap();
    assert!(!exports.is_empty());
    let exact = &exports[0];
    assert_eq!(exact["name"], "hashPassword");
    let lines = exact["lines"].as_array().unwrap();
    assert_eq!(lines[0], 3);
    assert_eq!(lines[1], 5);
}

// ---------------------------------------------------------------------------
// fmm_search — structured filters (depends_on, LOC range)
// ---------------------------------------------------------------------------

#[test]
fn search_depends_on_filter() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"depends_on": "config"}));

    let results = parsed.as_array().unwrap();
    let files: Vec<&str> = results
        .iter()
        .map(|r| r["file"].as_str().unwrap())
        .collect();
    assert!(files.contains(&"src/auth/session.ts"));
    assert!(files.contains(&"src/db/pool.ts"));
    assert_eq!(files.len(), 2);
}

#[test]
fn search_loc_range() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"min_loc": 7, "max_loc": 10}));

    let results = parsed.as_array().unwrap();
    let files: Vec<&str> = results
        .iter()
        .map(|r| r["file"].as_str().unwrap())
        .collect();
    // config.ts=10, pool.ts=10, crypto.ts=9 all match; session.ts=12, types.ts=6 don't
    assert!(files.contains(&"src/config.ts"));
    assert!(files.contains(&"src/db/pool.ts"));
    assert!(files.contains(&"src/utils/crypto.ts"));
    assert!(!files.contains(&"src/auth/session.ts"));
    assert!(!files.contains(&"src/auth/types.ts"));
}

#[test]
fn search_imports_filter() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"imports": "jwt"}));

    let results = parsed.as_array().unwrap();
    assert!(results
        .iter()
        .any(|r| r["file"].as_str().unwrap() == "src/auth/session.ts"));
}

#[test]
fn search_min_loc_filter() {
    let (_tmp, server) = setup_mcp_server();
    let parsed = call_tool_json(&server, "fmm_search", json!({"min_loc": 11}));

    let results = parsed.as_array().unwrap();
    assert!(results
        .iter()
        .any(|r| r["file"].as_str().unwrap() == "src/auth/session.ts"));
    // crypto.ts has loc: 9, should be excluded
    assert!(!results
        .iter()
        .any(|r| r["file"].as_str().unwrap() == "src/utils/crypto.ts"));
}
