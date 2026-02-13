use crate::parser::ExportEntry;
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

pub fn top_level_ancestor(node: tree_sitter::Node) -> tree_sitter::Node {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.parent().is_none() {
            return current;
        }
        current = parent;
    }
    current
}

pub fn collect_matches_with_lines(
    query: &Query,
    root_node: tree_sitter::Node,
    source_bytes: &[u8],
) -> Vec<ExportEntry> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root_node, source_bytes);
    while let Some(m) = iter.next() {
        for capture in m.captures {
            if let Ok(text) = capture.node.utf8_text(source_bytes) {
                let name = text.to_string();
                if seen.insert(name.clone()) {
                    let decl = top_level_ancestor(capture.node);
                    results.push(ExportEntry::new(
                        name,
                        decl.start_position().row + 1,
                        decl.end_position().row + 1,
                    ));
                }
            }
        }
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::{Language, Parser};

    fn setup_ts() -> (Language, Parser) {
        let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        (language, parser)
    }

    #[test]
    fn collect_matches_finds_all_captures() {
        let (lang, mut parser) = setup_ts();
        let source = "export function foo() {}\nexport function bar() {}";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (function_declaration name: (identifier) @name))",
        )
        .unwrap();

        let results = collect_matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(results, vec!["bar", "foo"]);
    }

    #[test]
    fn collect_matches_deduplicates() {
        let (lang, mut parser) = setup_ts();
        let source = "export { x } from './a';\nexport { x } from './b';";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (export_clause (export_specifier name: (identifier) @name)))",
        )
        .unwrap();

        let results = collect_matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(results, vec!["x"]);
    }

    #[test]
    fn collect_matches_empty_on_no_match() {
        let (lang, mut parser) = setup_ts();
        let source = "const x = 1;";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (function_declaration name: (identifier) @name))",
        )
        .unwrap();

        let results = collect_matches(&query, tree.root_node(), source.as_bytes());
        assert!(results.is_empty());
    }

    #[test]
    fn collect_matches_returns_sorted() {
        let (lang, mut parser) = setup_ts();
        let source =
            "export function zebra() {}\nexport function alpha() {}\nexport function middle() {}";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (function_declaration name: (identifier) @name))",
        )
        .unwrap();

        let results = collect_matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(results, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn collect_named_matches_filters_by_capture_name() {
        let (lang, mut parser) = setup_ts();
        let source = "export function foo() {}";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (function_declaration name: (identifier) @name))",
        )
        .unwrap();

        let results = collect_named_matches(&query, "name", tree.root_node(), source.as_bytes());
        assert_eq!(results, vec!["foo"]);
    }

    #[test]
    fn collect_named_matches_wrong_name_returns_empty() {
        let (lang, mut parser) = setup_ts();
        let source = "export function foo() {}";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (function_declaration name: (identifier) @name))",
        )
        .unwrap();

        let results =
            collect_named_matches(&query, "nonexistent", tree.root_node(), source.as_bytes());
        assert!(results.is_empty());
    }

    #[test]
    fn collect_named_matches_deduplicates_and_sorts() {
        let (lang, mut parser) = setup_ts();
        let source = "export { z } from './a';\nexport { a } from './b';\nexport { z } from './c';";
        let tree = parser.parse(source, None).unwrap();
        let query = Query::new(
            &lang,
            "(export_statement (export_clause (export_specifier name: (identifier) @name)))",
        )
        .unwrap();

        let results = collect_named_matches(&query, "name", tree.root_node(), source.as_bytes());
        assert_eq!(results, vec!["a", "z"]);
    }
}
