//! MCP tool integration tests.
//!
//! Every test calls through McpServer::call_tool — the real JSON-RPC path.
//! A shared fixture generates a SQLite index in a temp dir so the server loads
//! a realistic manifest with line ranges.
//!
//! Tools return sidecar-style YAML or CLI-style grouped text.

use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

/// Build an MCP server backed by a SQLite index in a temp dir.
fn setup_mcp_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/auth/session.ts",
        "import jwt from 'jwt';\nimport redis from 'redis';\nimport { Types } from './types';\nimport { Config } from '../config';\n\nexport function createSession() {\n  return jwt.sign({});\n}\n\nexport function validateSession(token: string) {\n  return jwt.verify(token);\n}\n",
    );

    write_file(
        root,
        "src/auth/types.ts",
        "export interface SessionToken {\n  token: string;\n  expires: number;\n}\n\nexport type UserRole = 'admin' | 'user';\n",
    );

    write_file(
        root,
        "src/config.ts",
        "import dotenv from 'dotenv';\n\nexport function loadConfig() {\n  dotenv.config();\n  return {};\n}\n\nexport interface AppConfig {\n  port: number;\n}\n",
    );

    write_file(
        root,
        "src/db/pool.ts",
        "import pg from 'pg';\nimport { Config } from '../config';\n\nexport class Pool {\n  private client: pg.Client;\n}\n\nexport function createPool() {\n  return new Pool();\n}\n",
    );

    write_file(
        root,
        "src/utils/crypto.ts",
        "import bcrypt from 'bcrypt';\n\nexport function hashPassword(pw: string) {\n  return bcrypt.hash(pw, 10);\n}\n\nexport function verifyPassword(pw: string, hash: string) {\n  return bcrypt.compare(pw, hash);\n}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

/// Call a tool and return the text content directly.
fn call_tool_text(server: &fmm::mcp::SqliteMcpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

/// Call a tool expecting an error response (text starts with "ERROR:").
fn call_tool_expect_error(server: &fmm::mcp::SqliteMcpServer, tool: &str, args: Value) -> String {
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
// Manifest loading (integration — validates DB discovery + load)
// ---------------------------------------------------------------------------

#[test]
fn manifest_loads_from_db() {
    let (tmp, _server) = setup_mcp_server();

    // TODO ALP-917: setup_mcp_server currently writes sidecars; migrate to generate().
    // Until migration, McpServer::with_root() falls back to sidecars if no DB.
    // This test verifies the manifest is loaded correctly (either path).
    let manifest = {
        use fmm_core::store::FmmStore;
        fmm_store::SqliteStore::open(tmp.path())
            .unwrap()
            .load_manifest()
            .unwrap()
    };
    assert_eq!(manifest.files.len(), 5);
}

#[test]
fn export_index_consistency() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = {
        use fmm_core::store::FmmStore;
        fmm_store::SqliteStore::open(tmp.path())
            .unwrap()
            .load_manifest()
            .unwrap()
    };
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

#[test]
fn read_symbol_truncate_false_bypasses_cap() {
    let (_tmp, server) = setup_mcp_server();
    // truncate: false — small symbol, same result, no truncation notice
    let text = call_tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "createSession", "truncate": false}),
    );
    assert!(
        text.contains("symbol: createSession"),
        "symbol header present"
    );
    assert!(
        !text.contains("[Truncated"),
        "no truncation notice with truncate=false; got: {text}"
    );
}

#[test]
fn read_symbol_truncate_true_is_default() {
    let (_tmp, server) = setup_mcp_server();
    // truncate: true (default) — small symbol still returns full content unchanged
    let text_default = call_tool_text(&server, "fmm_read_symbol", json!({"name": "createSession"}));
    let text_explicit = call_tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "createSession", "truncate": true}),
    );
    assert_eq!(
        text_default, text_explicit,
        "truncate: true matches default"
    );
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
// fmm_file_outline
// ---------------------------------------------------------------------------

#[test]
fn file_outline_returns_symbols() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_file_outline",
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
    assert!(
        text.contains("../config"),
        "dependencies must include ../config; got: {text}"
    );
    assert!(
        text.contains("./types"),
        "dependencies must include ./types; got: {text}"
    );
    assert!(text.contains("loc: 12"));
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

#[test]
fn dependency_graph_depth2_returns_depth_annotations() {
    let (_tmp, server) = setup_mcp_server();
    // depth=2 transitive traversal — output includes depth annotations
    let text = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/auth/session.ts", "depth": 2}),
    );
    assert!(
        text.contains("depth: 2"),
        "output should include depth header; got: {text}"
    );
    // local_deps section should appear with depth annotations
    assert!(
        text.contains("local_deps:"),
        "local_deps section present; got: {text}"
    );
    // src/auth/types.ts and src/config.ts are direct deps (depth 1)
    assert!(
        text.contains("src/auth/types.ts"),
        "types.ts in upstream; got: {text}"
    );
    assert!(
        text.contains("src/config.ts"),
        "config.ts in upstream; got: {text}"
    );
}

#[test]
fn dependency_graph_depth1_is_default_format() {
    let (_tmp, server) = setup_mcp_server();
    // depth=1 (default) should use backward-compatible format (no depth annotations)
    let text_default = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts"}),
    );
    let text_explicit = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "src/config.ts", "depth": 1}),
    );
    assert_eq!(text_default, text_explicit, "depth=1 matches default");
    assert!(
        !text_default.contains("depth:"),
        "depth=1 format has no depth annotation; got: {text_default}"
    );
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
    // config.ts itself has no dependency on config.ts — it should not appear as a result file.
    // Note: the header line may mention src/config.ts as the query target, so we check
    // specifically for it appearing as a result entry (at line start or after newline).
    let result_lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
    assert!(
        !result_lines
            .iter()
            .any(|l| l.trim_start().starts_with("src/config.ts")),
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
// fmm_search — combined term + structured filters (ALP-786)
// ---------------------------------------------------------------------------

#[test]
fn search_term_and_imports_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    // term="session" matches exports in session.ts AND types.ts (SessionToken).
    // imports="jwt" restricts to session.ts only.
    // Combined: only session.ts exports should appear.
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "session", "imports": "jwt"}),
    );

    assert!(
        text.contains("createSession"),
        "should include createSession"
    );
    assert!(
        text.contains("validateSession"),
        "should include validateSession"
    );
    assert!(
        !text.contains("SessionToken"),
        "SessionToken is in types.ts which doesn't import jwt"
    );
}

#[test]
fn search_term_and_min_loc_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    // term="hash" matches hashPassword (crypto.ts, LOC=9) and verifyPassword doesn't match.
    // min_loc=10 restricts to files with LOC >= 10 (config.ts=10, pool.ts=10, session.ts=12).
    // crypto.ts has LOC=9 so it's excluded — combined result should be empty exports.
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "hashPassword", "min_loc": 10}),
    );

    assert!(
        !text.contains("hashPassword"),
        "hashPassword is in crypto.ts (LOC=9) which fails min_loc=10 filter"
    );
}

#[test]
fn search_term_and_depends_on_filter_intersects() {
    let (_tmp, server) = setup_mcp_server();
    // term="session" matches createSession/validateSession (session.ts) and SessionToken (types.ts).
    // depends_on="config" restricts to files depending on config (session.ts, pool.ts).
    // types.ts has no config dependency so SessionToken should be excluded.
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"term": "session", "depends_on": "config"}),
    );

    assert!(
        text.contains("createSession"),
        "session.ts depends on config"
    );
    assert!(
        !text.contains("SessionToken"),
        "types.ts does not depend on config"
    );
}

#[test]
fn search_term_only_regression() {
    // Term-only queries must continue to work identically (no regression).
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"term": "createSession"}));

    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createSession"));
    assert!(text.contains("src/auth/session.ts"));
}

#[test]
fn search_filter_only_regression() {
    // Filter-only queries must continue to work identically (no regression).
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_search", json!({"imports": "jwt"}));

    assert!(text.contains("src/auth/session.ts"));
}

// ---------------------------------------------------------------------------
// ALP-823: fmm_search — multi-filter AND semantics
// ---------------------------------------------------------------------------

#[test]
fn search_export_and_imports_both_required() {
    // export="Pool" AND imports="pg" — only src/db/pool.ts matches both.
    // src/auth/session.ts imports jwt (not pg) and doesn't export Pool.
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"export": "Pool", "imports": "pg"}),
    );
    assert!(
        text.contains("src/db/pool.ts"),
        "pool.ts exports Pool AND imports pg; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/auth"),
        "session.ts should not appear (no Pool export); got:\n{}",
        text
    );
}

#[test]
fn search_export_and_min_loc_both_required() {
    // export="Pool" AND min_loc=50 — pool.ts is only 10 LOC, so nothing matches.
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"export": "Pool", "min_loc": 50}),
    );
    assert!(
        !text.contains("src/db/pool.ts"),
        "pool.ts (10 LOC) must not appear when min_loc=50; got:\n{}",
        text
    );
}

#[test]
fn search_imports_and_min_loc_both_required() {
    // imports="jwt" AND min_loc=10 — session.ts (12 LOC, imports jwt) matches.
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10}),
    );
    assert!(
        text.contains("src/auth/session.ts"),
        "session.ts matches imports=jwt AND min_loc>=10; got:\n{}",
        text
    );
}

#[test]
fn search_three_filters_and_semantics() {
    // imports="jwt" AND min_loc=10 AND export="createSession" → session.ts only.
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10, "export": "createSession"}),
    );
    assert!(
        text.contains("src/auth/session.ts"),
        "session.ts matches all three filters; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/db"),
        "db files must not appear; got:\n{}",
        text
    );
}

#[test]
fn search_three_filters_one_mismatch_returns_empty() {
    // imports="jwt" AND min_loc=10 AND export="Pool" — Pool is not in session.ts.
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(
        &server,
        "fmm_search",
        json!({"imports": "jwt", "min_loc": 10, "export": "Pool"}),
    );
    assert!(
        !text.contains("src/"),
        "no file satisfies all three filters; got:\n{}",
        text
    );
}

// ---------------------------------------------------------------------------
// Go module path resolution (ALP-738)
// ---------------------------------------------------------------------------

/// Build a server with a minimal Go multi-package project.
///
/// `cmd/main.go` imports `github.com/user/project/internal/handler`.
/// `internal/handler/handler.go` has no internal deps.
fn setup_go_mcp_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "cmd/main.go",
        "package main\n\nimport (\n\t\"fmt\"\n\t\"github.com/user/project/internal/handler\"\n)\n\nfunc main() {\n\tfmt.Println(handler.NewHandler())\n}\n",
    );

    write_file(
        root,
        "internal/handler/handler.go",
        "package handler\n\nimport \"net/http\"\n\ntype Handler struct{}\n\nfunc NewHandler() *Handler {\n\treturn &Handler{}\n}\n\nfunc (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
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

// ---------------------------------------------------------------------------
// ALP-822: fmm_read_symbol — redirect bare class reads to outline
// ---------------------------------------------------------------------------

/// Build an MCP server with a large class (> 10KB) that has methods.
fn setup_large_class_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Generate a class body that is definitely > 10KB.
    // Each method body is ~80 bytes; 150 methods ≈ 12KB.
    let mut source = String::from("export class BigService {\n");
    for i in 0..150usize {
        source.push_str(&format!(
            "  doWork{i:03}(input: string): string {{\n    // perform operation {i:03}\n    return input;\n  }}\n"
        ));
    }
    source.push_str("}\n");

    assert!(
        source.len() > 10_240,
        "test source must exceed 10KB, got {} bytes",
        source.len()
    );

    write_file(root, "src/service.ts", &source);

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn read_symbol_bare_class_over_cap_returns_redirect() {
    let (_tmp, server) = setup_large_class_server();
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "BigService"}));
    assert!(
        text.contains("would exceed the 10KB response cap"),
        "redirect message missing; got:\n{}",
        text
    );
    assert!(
        text.contains("methods:"),
        "method list missing in redirect; got:\n{}",
        text
    );
    assert!(
        text.contains("fmm_read_symbol(\"BigService.doWork"),
        "redirect hint missing; got:\n{}",
        text
    );
    assert!(
        text.contains("truncate: false"),
        "truncate: false hint missing; got:\n{}",
        text
    );
    // Must NOT contain actual source code
    assert!(
        !text.contains("return input"),
        "source code leaked into redirect; got:\n{}",
        text
    );
}

#[test]
fn read_symbol_bare_class_truncate_false_bypasses_redirect() {
    let (_tmp, server) = setup_large_class_server();
    let text = call_tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "BigService", "truncate": false}),
    );
    // With truncate: false we get full source, not the redirect
    assert!(
        !text.contains("would exceed"),
        "redirect should not fire with truncate: false; got:\n{}",
        text
    );
    assert!(
        text.contains("return input"),
        "full source expected with truncate: false; got:\n{}",
        text
    );
}

#[test]
fn read_symbol_small_class_no_redirect() {
    // A class small enough to fit under 10KB returns normal source
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "Pool"}));
    // Pool is a small class — should return source, not redirect
    assert!(
        !text.contains("would exceed"),
        "small class should not trigger redirect; got:\n{}",
        text
    );
    assert!(
        text.contains("Pool"),
        "class name should appear in source output; got:\n{}",
        text
    );
}

// ---------------------------------------------------------------------------
// ALP-845: fmm_lookup_export — collision disclosure
// ---------------------------------------------------------------------------

/// Build a server with two packages that both export `DispatchConfig`.
fn setup_collision_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "packages/renderer/dispatch.ts",
        "export interface DispatchConfig { timeout: number; }\n",
    );

    write_file(
        root,
        "packages/native/dispatch.ts",
        "export interface DispatchConfig { retries: number; }\n",
    );

    // Unique export — should produce no disclosure note.
    write_file(
        root,
        "packages/renderer/session.ts",
        "export function createSession() {}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

#[test]
fn lookup_export_collision_emits_disclosure_note() {
    let (_tmp, server) = setup_collision_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "DispatchConfig"}),
    );

    // Primary result is present
    assert!(
        text.contains("symbol: DispatchConfig"),
        "primary symbol missing:\n{}",
        text
    );
    // Disclosure note must appear
    assert!(
        text.contains("1 additional definition(s) found"),
        "collision disclosure missing:\n{}",
        text
    );
    assert!(
        text.contains("fmm_glossary"),
        "fmm_glossary reference missing from disclosure:\n{}",
        text
    );
}

#[test]
fn lookup_export_no_collision_no_disclosure() {
    let (_tmp, server) = setup_collision_server();
    let text = call_tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "createSession"}),
    );

    assert!(
        text.contains("symbol: createSession"),
        "symbol missing:\n{}",
        text
    );
    assert!(
        !text.contains("additional definition"),
        "unexpected collision note for unique export:\n{}",
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

#[test]
#[ignore]
fn debug_large_class_output() {
    let (tmp, server) = setup_large_class_server();
    let outline = call_tool_text(
        &server,
        "fmm_file_outline",
        json!({"file": "src/service.ts"}),
    );
    println!("OUTLINE:\n{}", &outline[..outline.len().min(500)]);
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "BigService"}));
    println!("READ_SYMBOL:\n{}", &text[..text.len().min(500)]);
    // Check method_index in manifest
    let manifest = {
        use fmm_core::store::FmmStore;
        fmm_store::SqliteStore::open(tmp.path())
            .unwrap()
            .load_manifest()
            .unwrap()
    };
    println!("METHOD_INDEX entries: {}", manifest.method_index.len());
    for (k, _) in manifest.method_index.iter().take(3) {
        println!("  {k}");
    }
}
