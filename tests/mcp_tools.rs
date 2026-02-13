//! MCP tool end-to-end tests.
//!
//! Tests each MCP tool handler with a real manifest built from temp fixtures.
//! Calls tool handlers directly — no JSON-RPC server needed.

use fmm::manifest::{FileEntry, Manifest};
use fmm::parser::{ExportEntry, Metadata};

fn e(name: &str) -> ExportEntry {
    ExportEntry::new(name.to_string(), 0, 0)
}

/// Build a test manifest with realistic data.
fn test_manifest() -> Manifest {
    let mut manifest = Manifest::new();

    manifest.add_file(
        "src/auth/session.ts",
        Metadata {
            exports: vec![e("createSession"), e("validateSession")],
            imports: vec!["jwt".into(), "redis".into()],
            dependencies: vec!["./types".into(), "../config".into()],
            loc: 234,
        },
    );

    manifest.add_file(
        "src/auth/types.ts",
        Metadata {
            exports: vec![e("SessionToken"), e("UserRole")],
            imports: vec![],
            dependencies: vec![],
            loc: 45,
        },
    );

    manifest.add_file(
        "src/config.ts",
        Metadata {
            exports: vec![e("loadConfig"), e("AppConfig")],
            imports: vec!["dotenv".into()],
            dependencies: vec![],
            loc: 120,
        },
    );

    manifest.add_file(
        "src/db/pool.ts",
        Metadata {
            exports: vec![e("Pool"), e("createPool")],
            imports: vec!["pg".into()],
            dependencies: vec!["../config".into()],
            loc: 89,
        },
    );

    manifest.add_file(
        "src/utils/crypto.ts",
        Metadata {
            exports: vec![e("hashPassword"), e("verifyPassword")],
            imports: vec!["bcrypt".into()],
            dependencies: vec![],
            loc: 67,
        },
    );

    manifest
}

// --- Helper to call tool handlers via McpServer's handle_request ---
// Since tool handlers are private, we test through the JSON-RPC handle_request path.
// We need to construct McpServer with our test manifest.

/// Build an MCP server with a pre-loaded manifest by writing sidecars to a temp dir
/// and letting the server load them.
fn setup_mcp_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let auth = src.join("auth");
    let db = src.join("db");
    let utils = src.join("utils");
    std::fs::create_dir_all(&auth).unwrap();
    std::fs::create_dir_all(&db).unwrap();
    std::fs::create_dir_all(&utils).unwrap();

    // Write source files and their v0.3 sidecars (with line ranges)
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

    // Build server from the temp directory (use with_root to avoid process-global cwd mutation)
    let server = fmm::mcp::McpServer::with_root(tmp.path().to_path_buf());

    (tmp, server)
}

fn write_source_and_sidecar(source_path: &std::path::Path, source: &str, sidecar: &str) {
    std::fs::write(source_path, source).unwrap();
    let mut sidecar_path = source_path.as_os_str().to_owned();
    sidecar_path.push(".fmm");
    std::fs::write(std::path::PathBuf::from(sidecar_path), sidecar).unwrap();
}

// The MCP tool handlers are private methods on McpServer.
// We test them by building a manifest and verifying it loads correctly,
// then test the manifest-based query logic that the tools use.

#[test]
fn manifest_loads_from_sidecars() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = fmm::manifest::Manifest::load_from_sidecars(tmp.path()).unwrap();
    assert!(manifest.files.contains_key("src/auth/session.ts"));
    assert!(manifest.files.contains_key("src/auth/types.ts"));
    assert!(manifest.files.contains_key("src/config.ts"));
    assert!(manifest.files.contains_key("src/db/pool.ts"));
    assert!(manifest.files.contains_key("src/utils/crypto.ts"));
    assert_eq!(manifest.files.len(), 5);
}

#[test]
fn lookup_export_finds_known_symbol() {
    let manifest = test_manifest();

    // Simulate fmm_lookup_export: look up "createSession"
    let file_path = manifest.export_index.get("createSession");
    assert_eq!(file_path, Some(&"src/auth/session.ts".to_string()));

    let entry = manifest.files.get("src/auth/session.ts").unwrap();
    assert_eq!(entry.loc, 234);
    assert!(entry.exports.contains(&"createSession".to_string()));
}

#[test]
fn lookup_export_returns_not_found() {
    let manifest = test_manifest();

    let result = manifest.export_index.get("nonexistentExport");
    assert!(result.is_none());
}

#[test]
fn list_exports_with_pattern() {
    let manifest = test_manifest();

    // Simulate fmm_list_exports with pattern "Session"
    let pattern = "session";
    let matches: Vec<(&String, &String)> = manifest
        .export_index
        .iter()
        .filter(|(name, _)| name.to_lowercase().contains(pattern))
        .collect();

    assert!(matches.iter().any(|(name, _)| *name == "createSession"));
    assert!(matches.iter().any(|(name, _)| *name == "validateSession"));
    assert!(matches.iter().any(|(name, _)| *name == "SessionToken"));
    assert_eq!(matches.len(), 3);
}

#[test]
fn list_exports_for_file() {
    let manifest = test_manifest();

    let entry = manifest.files.get("src/auth/session.ts").unwrap();
    assert_eq!(entry.exports, vec!["createSession", "validateSession"]);
}

#[test]
fn list_exports_file_not_found() {
    let manifest = test_manifest();
    assert!(!manifest.files.contains_key("src/nonexistent.ts"));
}

#[test]
fn file_info_returns_metadata() {
    let manifest = test_manifest();

    let entry = manifest.files.get("src/config.ts").unwrap();
    assert_eq!(entry.exports, vec!["loadConfig", "AppConfig"]);
    assert_eq!(entry.imports, vec!["dotenv"]);
    assert!(entry.dependencies.is_empty());
    assert_eq!(entry.loc, 120);
}

#[test]
fn dependency_graph_shows_upstream() {
    let manifest = test_manifest();

    let entry = manifest.files.get("src/auth/session.ts").unwrap();
    assert_eq!(entry.dependencies, vec!["./types", "../config"]);
}

#[test]
fn search_by_min_loc() {
    let manifest = test_manifest();

    let min_loc = 100;
    let matches: Vec<&String> = manifest
        .files
        .iter()
        .filter(|(_, entry)| entry.loc >= min_loc)
        .map(|(path, _)| path)
        .collect();

    assert!(matches.contains(&&"src/auth/session.ts".to_string())); // 234
    assert!(matches.contains(&&"src/config.ts".to_string())); // 120
    assert!(!matches.contains(&&"src/auth/types.ts".to_string())); // 45
}

#[test]
fn search_by_imports() {
    let manifest = test_manifest();

    let import_name = "pg";
    let matches: Vec<&String> = manifest
        .files
        .iter()
        .filter(|(_, entry)| entry.imports.iter().any(|i| i.contains(import_name)))
        .map(|(path, _)| path)
        .collect();

    assert_eq!(matches.len(), 1);
    assert!(matches.contains(&&"src/db/pool.ts".to_string()));
}

#[test]
fn search_by_dependency() {
    let manifest = test_manifest();

    let dep = "config";
    let matches: Vec<&String> = manifest
        .files
        .iter()
        .filter(|(_, entry)| entry.dependencies.iter().any(|d| d.contains(dep)))
        .map(|(path, _)| path)
        .collect();

    assert!(matches.contains(&&"src/auth/session.ts".to_string()));
    assert!(matches.contains(&&"src/db/pool.ts".to_string()));
    assert_eq!(matches.len(), 2);
}

#[test]
fn export_index_consistency() {
    let manifest = test_manifest();

    // Every export in the index should point to a file that actually has that export
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

#[test]
fn search_loc_range() {
    let manifest = test_manifest();

    let min_loc = 50;
    let max_loc = 100;
    let matches: Vec<(&String, &FileEntry)> = manifest
        .files
        .iter()
        .filter(|(_, entry)| entry.loc >= min_loc && entry.loc <= max_loc)
        .collect();

    // Should match: db/pool.ts (89), utils/crypto.ts (67)
    // Should NOT match: auth/session.ts (234), config.ts (120), auth/types.ts (45)
    assert_eq!(matches.len(), 2);
    let paths: Vec<&String> = matches.iter().map(|(p, _)| *p).collect();
    assert!(paths.contains(&&"src/db/pool.ts".to_string()));
    assert!(paths.contains(&&"src/utils/crypto.ts".to_string()));
}

// --- fmm_read_symbol tests ---

#[test]
fn read_symbol_returns_source_lines() {
    let (_tmp, server) = setup_mcp_server();
    let result = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "createSession"}),
        )
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["symbol"], "createSession");
    assert_eq!(parsed["file"], "src/auth/session.ts");
    assert_eq!(parsed["lines"], serde_json::json!([6, 8]));
    let source = parsed["source"].as_str().unwrap();
    assert!(source.contains("createSession"));
    assert!(!source.contains("validateSession"));
}

#[test]
fn read_symbol_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let result = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "nonExistent"}),
        )
        .unwrap();

    let is_error = result["isError"].as_bool().unwrap_or(false);
    assert!(is_error);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("not found"));
}

// --- fmm_file_outline tests ---

#[test]
fn file_outline_returns_symbols_with_lines() {
    let (_tmp, server) = setup_mcp_server();
    let result = server
        .call_tool(
            "fmm_file_outline",
            serde_json::json!({"file": "src/auth/session.ts"}),
        )
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["file"], "src/auth/session.ts");
    let symbols = parsed["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);

    assert_eq!(symbols[0]["name"], "createSession");
    assert_eq!(symbols[0]["lines"], serde_json::json!([6, 8]));
    assert_eq!(symbols[0]["size"], 3);

    assert_eq!(symbols[1]["name"], "validateSession");
    assert_eq!(symbols[1]["lines"], serde_json::json!([10, 12]));
    assert_eq!(symbols[1]["size"], 3);

    assert!(!parsed["imports"].as_array().unwrap().is_empty());
}

#[test]
fn file_outline_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let result = server
        .call_tool(
            "fmm_file_outline",
            serde_json::json!({"file": "src/nonexistent.ts"}),
        )
        .unwrap();

    let is_error = result["isError"].as_bool().unwrap_or(false);
    assert!(is_error);
}

#[test]
fn file_outline_shows_all_exports() {
    let (_tmp, server) = setup_mcp_server();
    let result = server
        .call_tool(
            "fmm_file_outline",
            serde_json::json!({"file": "src/utils/crypto.ts"}),
        )
        .unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    let symbols = parsed["symbols"].as_array().unwrap();
    let names: Vec<&str> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"hashPassword"));
    assert!(names.contains(&"verifyPassword"));
    assert_eq!(parsed["loc"], 9);
}
