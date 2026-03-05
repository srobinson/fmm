//! MCP tool integration tests.
//!
//! Every test calls through McpServer::call_tool — the real JSON-RPC path.
//! A shared fixture builds sidecars in a temp dir so the server loads a
//! realistic manifest with v0.3 line ranges.
//!
//! Tools now return sidecar-style YAML or CLI-style grouped text instead of JSON.

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

/// Call a tool and return the text content directly.
fn call_tool_text(server: &fmm::mcp::McpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

/// Call a tool expecting an error response (text starts with "ERROR:").
fn call_tool_expect_error(server: &fmm::mcp::McpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(
        text.starts_with("ERROR:"),
        "Expected ERROR: prefix but got: {}",
        text
    );
    text
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
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "createSession"}));

    // YAML header
    assert!(text.contains("symbol: createSession"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("lines: [6, 8]"));
    // Source code after second ---
    assert!(text.contains("createSession"));
    assert!(!text.contains("validateSession"));
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
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("symbols:"));
    assert!(text.contains("createSession: [6, 8]"));
    assert!(text.contains("validateSession: [10, 12]"));
    // Size comments
    assert!(text.contains("# 3 lines"));
    assert!(text.contains("imports: [jwt, redis]"));
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
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/utils/crypto.ts"}),
    );

    assert!(text.contains("hashPassword:"));
    assert!(text.contains("verifyPassword:"));
    assert!(text.contains("loc: 9"));
}

// ---------------------------------------------------------------------------
// fmm_file_info (alias for fmm_file_outline)
// ---------------------------------------------------------------------------

#[test]
fn file_info_delegates_to_file_outline() {
    // fmm_file_info is now an alias for fmm_file_outline — both must return
    // the same outline format (symbols: key, not exports:).
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_file_info",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.starts_with("---"));
    assert!(text.contains("file: src/auth/session.ts"));
    // outline format uses "symbols:" not "exports:"
    assert!(
        text.contains("symbols:"),
        "expected symbols: key; got: {text}"
    );
    assert!(text.contains("createSession:"));
    assert!(text.contains("validateSession:"));
    assert!(text.contains("imports: [jwt, redis]"));
    assert!(text.contains("dependencies: [./types, ../config]"));
    assert!(text.contains("loc: 12"));

    // Verify it returns identical output to fmm_file_outline
    let outline_text = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/auth/session.ts"}),
    );
    assert_eq!(
        text, outline_text,
        "fmm_file_info and fmm_file_outline must return identical output"
    );
}

// ---------------------------------------------------------------------------
// fmm_lookup_export
// ---------------------------------------------------------------------------

#[test]
fn lookup_export_returns_sidecar_yaml() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "createSession"}),
    );

    assert!(text.starts_with("---"));
    assert!(text.contains("symbol: createSession"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("lines: [6, 8]"));
    assert!(text.contains("exports:"));
    assert!(text.contains("imports: [jwt, redis]"));
    assert!(text.contains("loc: 12"));
}

#[test]
fn lookup_export_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(&server, "fmm_lookup_export", json!({"name": "nonExistent"}));
    assert!(text.contains("not found"));
}

// ---------------------------------------------------------------------------
// fmm_list_exports
// ---------------------------------------------------------------------------

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

    // Column-aligned text format
    assert!(text.contains("createSession"));
    assert!(text.contains("validateSession"));
    assert!(text.contains("SessionToken"));
    assert!(text.contains("src/auth/session.ts"));
}

#[test]
fn list_exports_all() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_list_exports", json!({}));

    // Multi-document YAML with inline export lists
    assert!(text.contains("---"));
    assert!(text.contains("file:"));
    assert!(text.contains("exports:"));
}

#[test]
fn list_exports_directory_filter_pattern() {
    let (_tmp, server) = setup_mcp_server();
    // Scoped to src/auth/ — should only return session.ts and types.ts exports
    let text = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "session", "directory": "src/auth/"}),
    );
    assert!(
        text.contains("createSession"),
        "createSession should appear; got: {text}"
    );
    // Pool is outside src/auth/ — should not appear
    assert!(
        !text.contains("Pool"),
        "Pool (from src/db/) should not appear with directory=src/auth/; got: {text}"
    );
}

#[test]
fn list_exports_directory_filter_all() {
    let (_tmp, server) = setup_mcp_server();
    // Scoped to src/db/ — only pool.ts should appear
    let text = call_tool_text(&server, "fmm_list_exports", json!({"directory": "src/db/"}));
    assert!(
        text.contains("Pool"),
        "Pool should appear under src/db/; got: {text}"
    );
    assert!(
        !text.contains("createSession"),
        "createSession (from src/auth/) should not appear; got: {text}"
    );
}

// ---------------------------------------------------------------------------
// fmm_list_exports — pagination tests (ALP-782)
// ---------------------------------------------------------------------------

#[test]
fn list_exports_pattern_pagination_limit_and_offset() {
    let (_tmp, server) = setup_mcp_server();

    // The fixture has 10 exports total. "S" (case-insensitive) matches:
    // SessionToken, UserRole (no), createSession, validateSession, AppConfig (no),
    // loadConfig (no), Pool (no), createPool (no), hashPassword (no), verifyPassword (no)
    // Actually let's use "" pattern won't work. Use "session" which matches 2 exports +
    // possibly method index. Use "e" to get a broader set. Actually just use limit=2
    // against all-matches for "a" to get at least 3 results.
    //
    // Fixture exports: createSession, validateSession, SessionToken, UserRole,
    //   loadConfig, AppConfig, Pool, createPool, hashPassword, verifyPassword
    // "a" matches: validateSession, SessionToken, loadConfig, AppConfig, hashPassword
    // That's 5. Test with limit=2.

    // First page: limit=2, offset=0
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

    // Second page: limit=2, offset=2
    let text2 = call_tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "a", "limit": 2, "offset": 2}),
    );
    assert!(
        text2.contains("showing: 3-4 of"),
        "second page header; got: {text2}"
    );

    // Last page: limit=10 — returns all, no pagination header
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

    // Fixture has 5 files with exports. Test with limit=2.

    // First page: limit=2, offset=0
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

    // Last page: limit=2, offset=4 — only 1 file remains, no "next" hint
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

    // No pagination header when all files fit
    let text3 = call_tool_text(&server, "fmm_list_exports", json!({"limit": 200}));
    assert!(
        !text3.contains("showing:"),
        "no header when all fit; got: {text3}"
    );
}

// ---------------------------------------------------------------------------
// fmm_dependency_graph
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_upstream_and_downstream() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/auth/session.ts"}),
    );

    assert!(text.contains("file: src/auth/session.ts"));
    // JS/TS relative deps are resolved to local file paths
    assert!(text.contains("local_deps:"), "got: {}", text);
    assert!(text.contains("src/auth/types.ts"), "got: {}", text);
    assert!(text.contains("src/config.ts"), "got: {}", text);
    assert!(text.contains("imports: [jwt, redis]"));
}

#[test]
fn dependency_graph_shows_downstream_dependents() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts"}),
    );

    assert!(text.contains("downstream:"));
    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("src/db/pool.ts"));
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
    let text = call_tool_text(&server, "fmm_search", json!({"term": "createSession"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("[6, 8]"));
}

#[test]
fn search_term_finds_fuzzy_exports() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "session"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("validateSession"));
    assert!(text.contains("SessionToken"));
}

#[test]
fn search_term_finds_file_path_matches() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "crypto"}));

    assert!(text.contains("FILES"));
    assert!(text.contains("crypto"));
}

#[test]
fn search_term_finds_import_matches() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "bcrypt"}));

    assert!(text.contains("IMPORTS"));
    assert!(text.contains("bcrypt"));
    assert!(text.contains("src/utils/crypto.ts"));
}

#[test]
fn search_term_returns_grouped_sections() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "config"}));

    // Should have grouped sections (may have EXPORTS, FILES, IMPORTS depending on matches)
    assert!(text.contains("config") || text.contains("Config"));
}

#[test]
fn search_term_case_insensitive() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "POOL"}));

    // Should find Pool/createPool exports and pool.ts file
    assert!(text.contains("Pool") || text.contains("createPool"));
    assert!(text.contains("pool"));
}

// ---------------------------------------------------------------------------
// fmm_search — export filter (exact + fuzzy fallback)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// fmm_search — line ranges in output
// ---------------------------------------------------------------------------

#[test]
fn search_results_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"export": "createSession"}));

    // Filter search returns per-file detail with export line ranges
    assert!(text.contains("[6, 8]"));
}

#[test]
fn search_term_exports_include_line_ranges() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "hashPassword"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("hashPassword"));
    assert!(text.contains("[3, 5]"));
}

// ---------------------------------------------------------------------------
// fmm_search — structured filters (depends_on, LOC range)
// ---------------------------------------------------------------------------

#[test]
fn search_depends_on_filter() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"depends_on": "config"}));

    assert!(text.contains("src/auth/session.ts"));
    assert!(text.contains("src/db/pool.ts"));
}

/// Regression test for ALP-758: depends_on with a full manifest path (src/config.ts) was
/// returning empty because the filter used a naive substring match on raw relative-path
/// dependency strings like "../config". dep_matches() must be used to resolve correctly.
#[test]
fn search_depends_on_full_manifest_path() {
    let (_tmp, server) = setup_mcp_server();
    // Pass the full manifest-relative path, not just a fragment.
    // "../config" (in session.ts) and "../config" (in pool.ts) must both resolve to src/config.ts.
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
    // config.ts itself has no dependency on config.ts — it should not appear
    assert!(
        !text.contains("src/config.ts\n") && !text.contains("src/config.ts "),
        "config.ts should not appear as a dependent of itself; got: {text}"
    );
}

#[test]
fn search_loc_range() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"min_loc": 7, "max_loc": 10}));

    // config.ts=10, pool.ts=10, crypto.ts=9 all match; session.ts=12, types.ts=6 don't
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

// ---------------------------------------------------------------------------
// Go module path resolution (ALP-738)
// ---------------------------------------------------------------------------

/// Build a server with a minimal Go multi-package project.
///
/// `cmd/main.go` imports `github.com/user/project/internal/handler`.
/// `internal/handler/handler.go` has no internal deps.
fn setup_go_mcp_server() -> (tempfile::TempDir, fmm::mcp::McpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let cmd = tmp.path().join("cmd");
    let handler_dir = tmp.path().join("internal").join("handler");
    std::fs::create_dir_all(&cmd).unwrap();
    std::fs::create_dir_all(&handler_dir).unwrap();

    write_source_and_sidecar(
        &cmd.join("main.go"),
        "package main\n\nimport (\n\t\"fmt\"\n\t\"github.com/user/project/internal/handler\"\n)\n\nfunc main() {\n\tfmt.Println(handler.NewHandler())\n}\n",
        "file: cmd/main.go\nfmm: v0.3\nexports:\n  main: [8, 10]\nimports: [fmt]\ndependencies: [github.com/user/project/internal/handler]\nloc: 10\n",
    );

    write_source_and_sidecar(
        &handler_dir.join("handler.go"),
        "package handler\n\nimport \"net/http\"\n\ntype Handler struct{}\n\nfunc NewHandler() *Handler {\n\treturn &Handler{}\n}\n\nfunc (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {}\n",
        // net/http in dependencies (not just imports) so dep resolution is exercised.
        // It must not match any project file — this is the stdlib false-positive guard.
        "file: internal/handler/handler.go\nfmm: v0.3\nexports:\n  Handler: [5, 5]\n  NewHandler: [7, 9]\nimports: [net/http]\ndependencies: [net/http]\nloc: 11\n",
    );

    let server = fmm::mcp::McpServer::with_root(tmp.path().to_path_buf());
    (tmp, server)
}

#[test]
fn go_internal_import_resolves_upstream() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "cmd/main.go"}),
    );

    // cmd/main.go depends on internal/handler/handler.go
    assert!(
        text.contains("local_deps:"),
        "expected local_deps in: {}",
        text
    );
    assert!(
        text.contains("internal/handler/handler.go"),
        "expected handler.go as upstream dep, got: {}",
        text
    );
}

#[test]
fn go_internal_import_resolves_downstream() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "internal/handler/handler.go"}),
    );

    // internal/handler/handler.go is depended on by cmd/main.go
    assert!(
        text.contains("downstream:"),
        "expected downstream: in: {}",
        text
    );
    assert!(
        text.contains("cmd/main.go"),
        "expected cmd/main.go as downstream dependent, got: {}",
        text
    );
}

#[test]
fn go_stdlib_import_no_false_positive() {
    let (_tmp, server) = setup_go_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "internal/handler/handler.go"}),
    );

    // handler.go has net/http in dependencies — stdlib must not resolve to any project file.
    // If it did, local_deps: would appear with a project file entry.
    assert!(
        !text.contains("local_deps:"),
        "net/http stdlib import caused false positive local dep: {}",
        text
    );
}
