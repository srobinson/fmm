use crate::support::{
    call_tool_expect_error, call_tool_text, setup_collision_server, setup_large_class_server,
    setup_mcp_server,
};
use fmm_core::store::FmmStore;
use serde_json::json;

#[test]
fn read_symbol_returns_source_lines() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "createSession"}));

    assert!(text.contains("symbol: createSession"));
    assert!(text.contains("file: src/auth/session.ts"));
    assert!(text.contains("lines: [6, 8]"));
    assert!(text.contains("createSession"));
    assert!(!text.contains("validateSession"));
}

#[test]
fn read_symbol_not_found() {
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_expect_error(&server, "fmm_read_symbol", json!({"name": "nonExistent"}));
    assert!(text.contains("not found"));
    assert!(
        text.contains("Use fmm_list_exports or fmm_search"),
        "MCP guidance should name MCP tools; got: {text}"
    );
    assert!(
        !text.contains("Use fmm exports or fmm search"),
        "MCP guidance should not name CLI commands; got: {text}"
    );
}

#[test]
fn read_symbol_duplicate_export_requires_file_qualified_name() {
    let (_tmp, server) = setup_collision_server();
    let text = call_tool_expect_error(
        &server,
        "fmm_read_symbol",
        json!({"name": "DispatchConfig"}),
    );

    assert!(
        text.contains("Symbol 'DispatchConfig' is ambiguous: 2 indexed exports use this name"),
        "got: {text}"
    );
    assert!(
        text.contains("fmm_read_symbol(name: \"packages/native/dispatch.ts:DispatchConfig\")"),
        "got: {text}"
    );
    assert!(
        text.contains("fmm_read_symbol(name: \"packages/renderer/dispatch.ts:DispatchConfig\")"),
        "got: {text}"
    );
}

#[test]
fn read_symbol_truncate_false_bypasses_cap() {
    let (_tmp, server) = setup_mcp_server();
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
    let (_tmp, server) = setup_mcp_server();
    let text = call_tool_text(&server, "fmm_read_symbol", json!({"name": "Pool"}));
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

    let manifest = fmm_store::SqliteStore::open(tmp.path())
        .unwrap()
        .load_manifest()
        .unwrap();
    println!("METHOD_INDEX entries: {}", manifest.method_index.len());
    for (key, _) in manifest.method_index.iter().take(3) {
        println!("  {key}");
    }
}
