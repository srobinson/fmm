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

    let result = cap_response(text, true);

    assert!(result.is_char_boundary(result.len()));
    assert!(result.contains("[Truncated"));
    assert!(
        result.contains("truncate: false"),
        "marker must reference truncate: false, got: {result}",
    );
}

#[test]
fn cap_response_passes_through_short_text() {
    let short = "hello world".to_string();
    assert_eq!(cap_response(short.clone(), true), short);
}

#[test]
fn cap_response_truncate_false_returns_full_text() {
    let large = "x\n".repeat(MAX_RESPONSE_BYTES);
    let result = cap_response(large.clone(), false);

    assert_eq!(result, large, "truncate=false must leave text unchanged");
    assert!(
        !result.contains("[Truncated"),
        "no truncation notice with truncate=false",
    );
}
