use super::tools::{glob_filename_matches, is_reexport_file};
use super::*;

#[test]
fn test_server_construction() {
    let server = McpServer::new();
    assert!(server.root.is_absolute() || server.root.as_os_str().is_empty());
}

#[test]
fn cap_response_handles_multibyte_utf8() {
    // Build a string that would split a multi-byte char at MAX_RESPONSE_BYTES
    let prefix = "x".repeat(McpServer::MAX_RESPONSE_BYTES - 1);
    // 4-byte emoji straddles the boundary
    let text = format!("{}🦀 and more text after", prefix);
    let result = McpServer::cap_response(text, true);
    assert!(result.is_char_boundary(result.len()));
    assert!(result.contains("[Truncated"));
    assert!(
        result.contains("truncate: false"),
        "marker must reference truncate: false, got: {}",
        result
    );
}

#[test]
fn cap_response_passes_through_short_text() {
    let short = "hello world".to_string();
    assert_eq!(McpServer::cap_response(short.clone(), true), short);
}

#[test]
fn cap_response_truncate_false_returns_full_text() {
    // Build a string larger than MAX_RESPONSE_BYTES
    let large = "x\n".repeat(McpServer::MAX_RESPONSE_BYTES);
    let result = McpServer::cap_response(large.clone(), false);
    assert_eq!(
        result, large,
        "truncate=false must return full text unchanged"
    );
    assert!(
        !result.contains("[Truncated"),
        "no truncation notice with truncate=false"
    );
}

#[test]
fn file_info_directory_path_returns_helpful_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool("fmm_file_outline", serde_json::json!({"file": "src/cli/"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {}",
        text
    );
}

#[test]
fn dependency_graph_directory_path_returns_helpful_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool(
            "fmm_dependency_graph",
            serde_json::json!({"file": "src/mcp/"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {}",
        text
    );
}

#[test]
fn read_symbol_empty_name_returns_helpful_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool("fmm_read_symbol", serde_json::json!({"name": ""}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
    assert!(
        text.contains("fmm_list_exports"),
        "should suggest fmm_list_exports, got: {}",
        text
    );
}

#[test]
fn file_outline_directory_path_returns_helpful_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool("fmm_file_outline", serde_json::json!({"file": "src/cli/"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.starts_with("ERROR:"),
        "expected ERROR: prefix, got: {}",
        text
    );
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {}",
        text
    );
}

#[test]
fn read_symbol_dotted_notation_returns_method_source() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("factory.ts");
    // Write a file with a class and a method
    std::fs::write(
        &file_path,
        "class NestFactoryStatic {\n  create() {\n    return 1;\n  }\n}\n",
    )
    .unwrap();

    let mut manifest = Manifest::new();
    manifest.add_file(
        "factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 5),
                ExportEntry::method("create".to_string(), 2, 4, "NestFactoryStatic".to_string()),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 5,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: dir.path().to_path_buf(),
    };

    // Dotted lookup returns the method
    let result = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "NestFactoryStatic.create"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.starts_with("ERROR:"),
        "expected success, got: {}",
        text
    );
    assert!(text.contains("create"), "should contain method body");
    assert!(
        text.contains("factory.ts"),
        "should contain file name, got: {}",
        text
    );

    // Flat lookup still works for the class
    let result2 = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "NestFactoryStatic"}),
        )
        .unwrap();
    let text2 = result2["content"][0]["text"].as_str().unwrap();
    assert!(
        !text2.starts_with("ERROR:"),
        "class lookup should succeed, got: {}",
        text2
    );
}

#[test]
fn read_symbol_dotted_not_found_gives_helpful_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool(
            "fmm_read_symbol",
            serde_json::json!({"name": "MyClass.missingMethod"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.starts_with("ERROR:"), "expected ERROR:, got: {}", text);
    assert!(
        text.contains("fmm_file_outline"),
        "should suggest fmm_file_outline, got: {}",
        text
    );
}

#[test]
fn is_reexport_file_detects_index_files() {
    assert!(is_reexport_file("agno/__init__.py"));
    assert!(is_reexport_file("src/index.ts"));
    assert!(is_reexport_file("src/index.tsx"));
    assert!(is_reexport_file("src/mod.rs"));
    assert!(is_reexport_file("libs/foo/index.js"));
    assert!(!is_reexport_file("agno/agent/agent.py"));
    assert!(!is_reexport_file("src/auth.ts"));
}

#[test]
fn read_symbol_follows_reexport_to_concrete_definition() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    // Create a temp dir with actual source files
    let dir = tempfile::tempdir().unwrap();
    let init_path = dir.path().join("agno").join("__init__.py");
    let agent_path = dir.path().join("agno").join("agent").join("agent.py");
    std::fs::create_dir_all(agent_path.parent().unwrap()).unwrap();

    // __init__.py re-exports Agent
    std::fs::write(
        &init_path,
        "from .agent.agent import Agent\n__all__ = ['Agent']\n",
    )
    .unwrap();

    // agent.py is the concrete definition with 5 lines
    let agent_src =
        "class Agent:\n    def __init__(self):\n        pass\n    def run(self):\n        pass\n";
    std::fs::write(&agent_path, agent_src).unwrap();

    let mut manifest = Manifest::new();
    // Index file re-exports Agent (no line range — typical for re-exports)
    manifest.add_file(
        "agno/__init__.py",
        Metadata {
            exports: vec![ExportEntry::new("Agent".to_string(), 1, 1)],
            imports: vec!["agno.agent.agent".to_string()],
            dependencies: vec![],
            loc: 2,
        },
    );
    // Concrete definition with proper line range
    manifest.add_file(
        "agno/agent/agent.py",
        Metadata {
            exports: vec![ExportEntry::new("Agent".to_string(), 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 5,
        },
    );

    // __init__.py wins the export_index (last writer wins), but we want agent.py
    let server = McpServer {
        manifest: Some(manifest),
        root: dir.path().to_path_buf(),
    };

    let result = server
        .call_tool("fmm_read_symbol", serde_json::json!({"name": "Agent"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();

    // Should resolve to the concrete definition file, not __init__.py
    assert!(
        text.contains("agno/agent/agent.py"),
        "should resolve to concrete definition, got: {}",
        text
    );
    assert!(
        !text.contains("__init__.py"),
        "should not use re-export site, got: {}",
        text
    );
    assert!(
        text.contains("class Agent"),
        "should include class body, got: {}",
        text
    );
}

#[test]
fn glob_filename_matches_star_ext() {
    assert!(glob_filename_matches("*.py", "agent.py"));
    assert!(glob_filename_matches("*.rs", "mod.rs"));
    assert!(!glob_filename_matches("*.py", "agent.rs"));
    assert!(!glob_filename_matches("*.py", "agent.pyc"));
}

#[test]
fn glob_filename_matches_prefix_star() {
    assert!(glob_filename_matches("test_*", "test_agent.py"));
    assert!(glob_filename_matches("test_*", "test_.py"));
    assert!(!glob_filename_matches("test_*", "mytest_agent.py"));
}

#[test]
fn glob_filename_matches_literal() {
    assert!(glob_filename_matches("mod.rs", "mod.rs"));
    assert!(!glob_filename_matches("mod.rs", "mod.ts"));
}

#[test]
fn glob_filename_matches_star_wildcard() {
    assert!(glob_filename_matches("*", "anything.py"));
    assert!(glob_filename_matches("*", ""));
}

#[test]
fn list_files_tool_no_args() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/a.rs",
        Metadata {
            exports: vec![ExportEntry::new("Foo".to_string(), 1, 10)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        },
    );
    manifest.add_file(
        "src/b.rs",
        Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool("fmm_list_files", serde_json::json!({}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("total: 2"),
        "expected total: 2, got: {}",
        text
    );
    assert!(text.contains("src/a.rs"));
    assert!(text.contains("src/b.rs"));
}

#[test]
fn list_files_tool_with_directory() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/cli/mod.rs",
        Metadata {
            exports: vec![ExportEntry::new("Cli".to_string(), 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 30,
        },
    );
    manifest.add_file(
        "src/mcp/mod.rs",
        Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 100,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"directory": "src/cli/"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("total: 1"), "got: {}", text);
    assert!(text.contains("src/cli/mod.rs"));
    assert!(!text.contains("src/mcp/mod.rs"));
}

#[test]
fn list_files_tool_pagination_limit_and_offset() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    for i in 1..=5 {
        manifest.add_file(
            &format!("src/mod{i}.rs"),
            Metadata {
                exports: vec![ExportEntry::new(format!("Item{i}"), 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );
    }

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    // First page: limit=2, offset=0 — should show src/mod1.rs and src/mod2.rs
    // Use sort_by=name to get deterministic order (all files have equal LOC).
    let result = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"limit": 2, "offset": 0, "sort_by": "name"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("total: 5"), "total wrong; got: {}", text);
    assert!(
        text.contains("showing: 1-2 of 5"),
        "pagination header wrong; got: {}",
        text
    );
    assert!(
        text.contains("offset=2"),
        "next-page hint missing; got: {}",
        text
    );
    assert!(text.contains("src/mod1.rs"));
    assert!(text.contains("src/mod2.rs"));
    assert!(!text.contains("src/mod3.rs"));

    // Second page: limit=2, offset=2 — should show src/mod3.rs and src/mod4.rs
    let result2 = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"limit": 2, "offset": 2, "sort_by": "name"}),
        )
        .unwrap();
    let text2 = result2["content"][0]["text"].as_str().unwrap();
    assert!(text2.contains("total: 5"), "got: {}", text2);
    assert!(text2.contains("src/mod3.rs"));
    assert!(text2.contains("src/mod4.rs"));
    assert!(!text2.contains("src/mod1.rs"));

    // Last page: offset=4, limit=2 — only src/mod5.rs, no "next" hint
    let result3 = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"limit": 2, "offset": 4, "sort_by": "name"}),
        )
        .unwrap();
    let text3 = result3["content"][0]["text"].as_str().unwrap();
    assert!(text3.contains("src/mod5.rs"));
    assert!(
        !text3.contains("offset=6"),
        "should not show next hint on last page; got: {}",
        text3
    );

    // Out-of-bounds offset: should return empty files but NOT a bad "showing: N+1-N of M" line
    let result_oob = server
        .call_tool(
            "fmm_list_files",
            serde_json::json!({"limit": 2, "offset": 1000}),
        )
        .unwrap();
    let text_oob = result_oob["content"][0]["text"].as_str().unwrap();
    assert!(
        text_oob.contains("total: 5"),
        "total should still appear; got: {}",
        text_oob
    );
    assert!(
        !text_oob.contains("showing:"),
        "showing line must not appear for out-of-bounds offset; got: {}",
        text_oob
    );
}

// --- ALP-803: fmm_list_files sort_by + order ---

fn list_files_sort_manifest() -> McpServer {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/alpha.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("A".to_string(), 1, 5),
                ExportEntry::new("B".to_string(), 6, 10),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 100,
        },
    );
    manifest.add_file(
        "src/beta.ts",
        Metadata {
            exports: vec![ExportEntry::new("C".to_string(), 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 30,
        },
    );
    manifest.add_file(
        "src/gamma.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("D".to_string(), 1, 5),
                ExportEntry::new("E".to_string(), 6, 10),
                ExportEntry::new("F".to_string(), 11, 15),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 60,
        },
    );
    McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    }
}

fn list_files_order(server: &McpServer, args: serde_json::Value) -> Vec<String> {
    let result = server.call_tool("fmm_list_files", args).unwrap();
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    // Output format: "  - src/alpha.ts   # loc: 100, exports: 2"
    text.lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .map(|l| {
            l.trim_start()
                .strip_prefix("- ")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

#[test]
fn list_files_default_sort_is_loc_desc() {
    let server = list_files_sort_manifest();
    let order = list_files_order(&server, serde_json::json!({}));
    assert_eq!(
        order,
        vec!["src/alpha.ts", "src/gamma.ts", "src/beta.ts"],
        "default sort should be LOC descending (largest first), got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_loc_defaults_to_desc() {
    let server = list_files_sort_manifest();
    let order = list_files_order(&server, serde_json::json!({"sort_by": "loc"}));
    assert_eq!(
        order,
        vec!["src/alpha.ts", "src/gamma.ts", "src/beta.ts"],
        "sort_by=loc should default to desc (largest first), got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_loc_asc() {
    let server = list_files_sort_manifest();
    let order = list_files_order(
        &server,
        serde_json::json!({"sort_by": "loc", "order": "asc"}),
    );
    assert_eq!(
        order,
        vec!["src/beta.ts", "src/gamma.ts", "src/alpha.ts"],
        "sort_by=loc order=asc should return smallest first, got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_exports_defaults_to_desc() {
    let server = list_files_sort_manifest();
    let order = list_files_order(&server, serde_json::json!({"sort_by": "exports"}));
    assert_eq!(
        order,
        vec!["src/gamma.ts", "src/alpha.ts", "src/beta.ts"],
        "sort_by=exports should default to desc (most exports first), got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_name_desc() {
    let server = list_files_sort_manifest();
    let order = list_files_order(
        &server,
        serde_json::json!({"sort_by": "name", "order": "desc"}),
    );
    assert_eq!(
        order,
        vec!["src/gamma.ts", "src/beta.ts", "src/alpha.ts"],
        "sort_by=name order=desc should reverse alphabetical, got: {:?}",
        order
    );
}

#[test]
fn list_files_invalid_sort_by_returns_error() {
    let server = list_files_sort_manifest();
    let result = server.call_tool("fmm_list_files", serde_json::json!({"sort_by": "invalid"}));
    let text = result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.starts_with("ERROR:"),
        "invalid sort_by must return ERROR:, got: {}",
        text
    );
    assert!(
        text.contains("sort_by"),
        "error message should mention sort_by, got: {}",
        text
    );
}

#[test]
fn list_files_invalid_order_returns_error() {
    let server = list_files_sort_manifest();
    let result = server.call_tool(
        "fmm_list_files",
        serde_json::json!({"sort_by": "loc", "order": "random"}),
    );
    let text = result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.starts_with("ERROR:"),
        "invalid order must return ERROR:, got: {}",
        text
    );
    assert!(
        text.contains("order"),
        "error message should mention order, got: {}",
        text
    );
}

// --- ALP-818: fmm_list_files group_by=subdir rollup ---

#[test]
fn list_files_group_by_subdir_buckets_files_by_immediate_dir() {
    let server = list_files_sort_manifest(); // alpha(100), beta(30), gamma(60) all under src/
    let result = server
        .call_tool("fmm_list_files", serde_json::json!({"group_by": "subdir"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    // All three files are directly under src/ — one bucket "src/"
    assert!(
        text.contains("src/"),
        "should show src/ bucket; got:\n{}",
        text
    );
    assert!(
        text.contains("3 files"),
        "bucket should show 3 files; got:\n{}",
        text
    );
    assert!(
        text.contains("190 LOC"),
        "bucket should show 190 total LOC; got:\n{}",
        text
    );
    assert!(
        text.contains("summary:"),
        "summary line should appear; got:\n{}",
        text
    );
}

#[test]
fn list_files_group_by_invalid_returns_error() {
    let server = list_files_sort_manifest();
    let result = server.call_tool("fmm_list_files", serde_json::json!({"group_by": "unknown"}));
    let text = result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.starts_with("ERROR:"),
        "invalid group_by must return ERROR:; got: {}",
        text
    );
}

// --- ALP-819: fmm_list_files filter=source / filter=tests ---

#[test]
fn list_files_filter_source_excludes_test_files() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/app.service.ts",
        Metadata {
            exports: vec![ExportEntry::new("AppService".to_string(), 1, 20)],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        },
    );
    manifest.add_file(
        "src/app.spec.ts",
        Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 15,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool("fmm_list_files", serde_json::json!({"filter": "source"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("src/app.service.ts"),
        "source filter should include service file; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/app.spec.ts"),
        "source filter should exclude spec file; got:\n{}",
        text
    );
    assert!(
        text.contains("total: 1"),
        "total should be 1; got:\n{}",
        text
    );
}

#[test]
fn list_files_filter_tests_returns_only_test_files() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/app.service.ts",
        Metadata {
            exports: vec![ExportEntry::new("AppService".to_string(), 1, 20)],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        },
    );
    manifest.add_file(
        "src/app.spec.ts",
        Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 15,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool("fmm_list_files", serde_json::json!({"filter": "tests"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("src/app.spec.ts"),
        "tests filter should include spec file; got:\n{}",
        text
    );
    assert!(
        !text.contains("src/app.service.ts"),
        "tests filter should exclude service file; got:\n{}",
        text
    );
    assert!(
        text.contains("total: 1"),
        "total should be 1; got:\n{}",
        text
    );
}

#[test]
fn list_files_filter_invalid_returns_error() {
    let server = list_files_sort_manifest();
    let result = server
        .call_tool("fmm_list_files", serde_json::json!({"filter": "bogus"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.starts_with("ERROR:"),
        "invalid filter must return ERROR:; got: {}",
        text
    );
}

// --- ALP-821: fmm_list_files sort_by=modified ---

fn list_files_modified_manifest() -> McpServer {
    use crate::manifest::{FileEntry, Manifest};
    let mut manifest = Manifest::new();
    // Insert directly so we can set modified dates
    manifest.files.insert(
        "src/alpha.ts".to_string(),
        FileEntry {
            exports: vec!["A".to_string()],
            export_lines: None,
            methods: None,
            imports: vec![],
            dependencies: vec![],
            loc: 100,
            modified: Some("2026-03-01".to_string()),
        },
    );
    manifest.files.insert(
        "src/beta.ts".to_string(),
        FileEntry {
            exports: vec!["B".to_string()],
            export_lines: None,
            methods: None,
            imports: vec![],
            dependencies: vec![],
            loc: 30,
            modified: Some("2026-03-05".to_string()),
        },
    );
    manifest.files.insert(
        "src/gamma.ts".to_string(),
        FileEntry {
            exports: vec!["C".to_string()],
            export_lines: None,
            methods: None,
            imports: vec![],
            dependencies: vec![],
            loc: 60,
            modified: Some("2026-02-20".to_string()),
        },
    );
    McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    }
}

#[test]
fn list_files_sort_by_modified_defaults_to_desc() {
    let server = list_files_modified_manifest();
    let order = list_files_order(&server, serde_json::json!({"sort_by": "modified"}));
    assert_eq!(
        order,
        vec!["src/beta.ts", "src/alpha.ts", "src/gamma.ts"],
        "sort_by=modified should default to desc (most recent first), got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_modified_asc() {
    let server = list_files_modified_manifest();
    let order = list_files_order(
        &server,
        serde_json::json!({"sort_by": "modified", "order": "asc"}),
    );
    assert_eq!(
        order,
        vec!["src/gamma.ts", "src/alpha.ts", "src/beta.ts"],
        "sort_by=modified order=asc should return oldest first, got: {:?}",
        order
    );
}

#[test]
fn list_files_sort_by_modified_shows_date_in_output() {
    let server = list_files_modified_manifest();
    let result = server
        .call_tool("fmm_list_files", serde_json::json!({"sort_by": "modified"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("modified: 2026-03-05"),
        "output should show modified date, got:\n{}",
        text
    );
}

#[test]
fn list_files_sort_by_modified_composable_with_filter() {
    let server = list_files_modified_manifest();
    let order = list_files_order(
        &server,
        serde_json::json!({"sort_by": "modified", "directory": "src/"}),
    );
    assert_eq!(
        order,
        vec!["src/beta.ts", "src/alpha.ts", "src/gamma.ts"],
        "sort_by=modified + directory filter should work, got: {:?}",
        order
    );
}

// --- ALP-778: fmm_lookup_export dotted name fallback ---

#[test]
fn lookup_export_dotted_name_resolves_via_method_index() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 200),
                ExportEntry::method(
                    "createApplicationContext".to_string(),
                    166,
                    195,
                    "NestFactoryStatic".to_string(),
                ),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 200,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    // Dotted lookup resolves via method_index
    let result = server
        .call_tool(
            "fmm_lookup_export",
            serde_json::json!({"name": "NestFactoryStatic.createApplicationContext"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.starts_with("ERROR:"),
        "expected success, got: {}",
        text
    );
    assert!(
        text.contains("src/factory.ts"),
        "should contain file, got: {}",
        text
    );
    assert!(
        text.contains("166"),
        "should contain start line, got: {}",
        text
    );
    assert!(
        text.contains("195"),
        "should contain end line, got: {}",
        text
    );
}

#[test]
fn lookup_export_flat_name_still_works_after_method_index_added() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![ExportEntry::new("NestFactoryStatic".to_string(), 1, 200)],
            imports: vec![],
            dependencies: vec![],
            loc: 200,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool(
            "fmm_lookup_export",
            serde_json::json!({"name": "NestFactoryStatic"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.starts_with("ERROR:"),
        "flat lookup should succeed, got: {}",
        text
    );
    assert!(text.contains("src/factory.ts"), "got: {}", text);
}

#[test]
fn lookup_export_unknown_dotted_name_returns_error() {
    use crate::manifest::Manifest;
    let server = McpServer {
        manifest: Some(Manifest::new()),
        root: std::path::PathBuf::from("/tmp"),
    };
    let result = server
        .call_tool(
            "fmm_lookup_export",
            serde_json::json!({"name": "MyClass.ghostMethod"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.starts_with("ERROR:"), "expected ERROR:, got: {}", text);
}

// --- ALP-779: fmm_list_exports pattern includes method matches ---

#[test]
fn list_exports_pattern_includes_method_index_matches() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 200),
                ExportEntry::method(
                    "create".to_string(),
                    79,
                    113,
                    "NestFactoryStatic".to_string(),
                ),
                ExportEntry::method(
                    "createApplicationContext".to_string(),
                    166,
                    195,
                    "NestFactoryStatic".to_string(),
                ),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 200,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    let result = server
        .call_tool("fmm_list_exports", serde_json::json!({"pattern": "create"}))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.starts_with("ERROR:"),
        "expected success, got: {}",
        text
    );
    assert!(
        text.contains("NestFactoryStatic.create"),
        "should contain dotted method match, got: {}",
        text
    );
    assert!(
        text.contains("NestFactoryStatic.createApplicationContext"),
        "should contain second dotted method, got: {}",
        text
    );
}

#[test]
fn list_exports_pattern_directory_filter_applies_to_methods() {
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![ExportEntry::method(
                "create".to_string(),
                79,
                113,
                "MyFactory".to_string(),
            )],
            imports: vec![],
            dependencies: vec![],
            loc: 113,
        },
    );
    manifest.add_file(
        "lib/other.ts",
        Metadata {
            exports: vec![ExportEntry::method(
                "create".to_string(),
                1,
                5,
                "OtherFactory".to_string(),
            )],
            imports: vec![],
            dependencies: vec![],
            loc: 5,
        },
    );

    let server = McpServer {
        manifest: Some(manifest),
        root: std::path::PathBuf::from("/tmp"),
    };

    // Directory filter: only src/ methods should appear
    let result = server
        .call_tool(
            "fmm_list_exports",
            serde_json::json!({"pattern": "create", "directory": "src/"}),
        )
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("MyFactory.create"),
        "should contain src method, got: {}",
        text
    );
    assert!(
        !text.contains("OtherFactory.create"),
        "should not contain lib method, got: {}",
        text
    );
}
