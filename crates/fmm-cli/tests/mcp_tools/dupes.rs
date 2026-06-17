use super::support::{call_tool_text, setup_similarity_server};
use serde_json::{Value, json};

#[test]
fn dupe_clusters_surfaces_clone_not_coincidence() {
    let (_tmp, server) = setup_similarity_server();

    let text = call_tool_text(&server, "fmm_dupe_clusters", json!({"min_score": 0.6}));
    let payload: Value = serde_json::from_str(&text).expect("dupe clusters should be JSON");
    let rendered = serde_json::to_string(&payload).unwrap();

    assert!(
        rendered.contains("collectImports"),
        "real clone must surface, got: {rendered}"
    );
    assert!(
        rendered.contains("extractImports"),
        "real clone must surface, got: {rendered}"
    );
    assert!(
        !rendered.contains("collectImportsFromSpec"),
        "spec file clone must be excluded by default, got: {rendered}"
    );
}

#[test]
fn dupe_clusters_include_tests_restores_test_candidates() {
    let (_tmp, server) = setup_similarity_server();

    let text = call_tool_text(
        &server,
        "fmm_dupe_clusters",
        json!({"include_tests": true, "min_score": 0.6}),
    );

    assert!(
        text.contains("collectImportsFromSpec"),
        "spec file clone should surface with include_tests, got: {text}"
    );
}
