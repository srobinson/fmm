use std::collections::HashSet;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

/// Collect unique text from all captures of a query, returned as a sorted Vec.
pub fn collect_matches(
    query: &Query,
    root_node: tree_sitter::Node,
    source_bytes: &[u8],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root_node, source_bytes);
    while let Some(m) = iter.next() {
        for capture in m.captures {
            if let Ok(text) = capture.node.utf8_text(source_bytes) {
                seen.insert(text.to_string());
            }
        }
    }
    let mut results: Vec<String> = seen.into_iter().collect();
    results.sort();
    results
}

/// Collect unique text from captures matching a specific capture name in a query.
/// Returns None for captures whose index is out of bounds (safe).
pub fn collect_named_matches(
    query: &Query,
    capture_name: &str,
    root_node: tree_sitter::Node,
    source_bytes: &[u8],
) -> Vec<String> {
    let capture_names = query.capture_names();
    let mut seen = HashSet::new();
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root_node, source_bytes);
    while let Some(m) = iter.next() {
        for capture in m.captures {
            let idx = capture.index as usize;
            if idx < capture_names.len() && capture_names[idx] == capture_name {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    seen.insert(text.to_string());
                }
            }
        }
    }
    let mut results: Vec<String> = seen.into_iter().collect();
    results.sort();
    results
}
