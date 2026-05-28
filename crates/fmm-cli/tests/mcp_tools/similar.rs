use super::support::{call_tool_text, setup_similarity_server};
use serde_json::json;

/// A real clone (shared name token + identical shape) must surface; a
/// coincidental same-shape-different-job symbol must be threshold-gated out.
/// This is the precision regression lock — if it fails, tune the scorer or
/// threshold, do not loosen the assertion.
#[test]
fn find_similar_surfaces_clone_not_coincidence() {
    let (_tmp, server) = setup_similarity_server();

    let text = call_tool_text(&server, "fmm_find_similar", json!({"name": "extractImports"}));

    assert!(
        text.contains("collectImports"),
        "real clone must surface, got: {text}"
    );
    assert!(
        !text.contains("isReady"),
        "coincidental shape must be gated, got: {text}"
    );
    assert!(
        !text.contains("isDone"),
        "coincidental shape must be gated, got: {text}"
    );
}

/// Unknown probe name returns the no-match line, not an error.
#[test]
fn find_similar_unknown_probe_reports_no_match() {
    let (_tmp, server) = setup_similarity_server();

    let text = call_tool_text(
        &server,
        "fmm_find_similar",
        json!({"name": "thisDoesNotExistAnywhere"}),
    );

    assert!(
        text.contains("No similar symbols found"),
        "expected no-match line, got: {text}"
    );
}
