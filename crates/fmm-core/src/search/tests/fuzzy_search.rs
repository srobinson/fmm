use crate::manifest::Manifest;
use crate::parser::{ExportEntry, Metadata};

use super::super::bare_search;
use super::super::helpers::export_match_score;
use super::support::manifest_with;

#[test]
fn export_match_score_exact() {
    assert_eq!(export_match_score("Agent", "agent"), 100);
}

#[test]
fn export_match_score_prefix() {
    assert_eq!(export_match_score("AgentConfig", "agent"), 80);
}

#[test]
fn export_match_score_suffix() {
    assert_eq!(export_match_score("MockAgent", "agent"), 60);
}

#[test]
fn export_match_score_word_boundary() {
    assert_eq!(export_match_score("run_agent_loop", "agent"), 40);
}

#[test]
fn export_match_score_substring() {
    assert_eq!(export_match_score("buckets_handler", "ck"), 20);
}

#[test]
fn bare_search_scores_prefix_before_suffix() {
    let manifest = manifest_with(vec![
        ("src/mock_agent.py", vec![]),
        ("src/agent_config.py", vec![]),
    ]);
    let result = bare_search(&manifest, "agent", None);
    let names: Vec<&str> = result.exports.iter().map(|h| h.name.as_str()).collect();
    if let (Some(ag_pos), Some(mock_pos)) = (
        names
            .iter()
            .position(|&n| n.contains("AgentConfig") || n == "agent_config.py"),
        names
            .iter()
            .position(|&n| n.contains("MockAgent") || n == "mock_agent.py"),
    ) {
        assert!(
            ag_pos <= mock_pos,
            "Expected prefix match before suffix match, got: {:?}",
            names
        );
    }
}

#[test]
fn bare_search_limit_caps_results() {
    let mut manifest = Manifest::new();
    for i in 0..10 {
        manifest.add_file(
            &format!("src/mod{}.py", i),
            Metadata {
                exports: vec![ExportEntry::new(format!("FooHandler{}", i), 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
                ..Default::default()
            },
        );
    }
    let result = bare_search(&manifest, "foo", Some(3));
    assert!(
        result.exports.len() <= 3,
        "expected at most 3 results, got {}",
        result.exports.len()
    );
    assert!(
        result.total_exports.is_some(),
        "should report total when capped"
    );
    assert_eq!(result.total_exports.unwrap(), 10);
}
