use super::super::{MAX_RESPONSE_BYTES, McpServer, cap_response};

#[test]
fn test_server_construction() {
    let server = McpServer::new();
    assert!(server.root.is_absolute() || server.root.as_os_str().is_empty());
}

#[test]
fn cap_response_handles_multibyte_utf8() {
    let prefix = "x".repeat(MAX_RESPONSE_BYTES - 1);
    let text = format!("{prefix}é and more text after");

    let result = cap_response(text, true, true);

    assert!(result.is_char_boundary(result.len()));
    assert!(result.contains("[Truncated"));
    assert!(
        result.contains("truncate: false to get the full response"),
        "marker must reference the generic MCP response escape hatch, got: {result}",
    );
}

#[test]
fn cap_response_passes_through_short_text() {
    let short = "hello world".to_string();
    assert_eq!(cap_response(short.clone(), true, true), short);
}

#[test]
fn cap_response_truncate_false_returns_full_text() {
    let large = "x\n".repeat(MAX_RESPONSE_BYTES);
    let result = cap_response(large.clone(), false, true);

    assert_eq!(result, large, "truncate=false must leave text unchanged");
    assert!(
        !result.contains("[Truncated"),
        "no truncation notice with truncate=false",
    );
}

#[test]
fn cap_response_omits_escape_hatch_for_tools_without_truncate() {
    let large = "x\n".repeat(MAX_RESPONSE_BYTES);
    let result = cap_response(large, true, false);

    assert!(result.contains("[Truncated"));
    assert!(
        !result.contains("truncate: false"),
        "tools without a truncate parameter must not advertise it; got: {result}",
    );
}
