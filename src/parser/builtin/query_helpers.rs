//! Shared tree-sitter query utilities used across all builtin parsers.
//!
//! # Capture name conventions
//!
//! All fmm parsers follow a shared set of capture names so these helpers work
//! consistently. The full reference is in `docs/QUERIES.md`. Quick summary:
//!
//! | Capture | Meaning |
//! |---|---|
//! | `@name` | Primary identifier being defined (function, class, variable) |
//! | `@vis` | Visibility modifier (`pub`, `public`, `export`) |
//! | `@source` | Import/export source path string |
//! | `@class_name` | Parent class identifier (for method extraction) |
//! | `@method_name` | Method identifier (paired with `@class_name`) |
//! | `@attr_name` | Decorator or annotation name |
//! | `@original_name` | Pre-alias export name (TS: `export { foo as bar }`) |
//! | `@values` | List literal (Python `__all__`) |
//!
//! # Choosing a helper
//!
//! - [`collect_matches_with_lines`] — use for exports; returns [`ExportEntry`] with line ranges.
//! - [`collect_named_matches`] — use when a query has multiple capture names and you want one.
//! - [`collect_matches`] — simplest form; returns deduplicated strings from any capture.
//! - [`collect_captures`] — generic building block; bring your own `T` and extractor closure.

use crate::parser::ExportEntry;
use anyhow::Result;
use std::collections::HashSet;
use std::hash::Hash;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

/// Create and configure a [`TSParser`] for the given language.
///
/// Replaces the repeated three-line init block across every parser `new()`:
/// ```ignore
/// let mut parser = TSParser::new();
/// parser.set_language(&language)
///     .map_err(|e| anyhow::anyhow!("Failed to set {} language: {}", lang, e))?;
/// ```
pub fn make_parser(language: &Language, lang_name: &str) -> Result<TSParser> {
    let mut parser = TSParser::new();
    parser
        .set_language(language)
        .map_err(|e| anyhow::anyhow!("Failed to set {} language: {}", lang_name, e))?;
    Ok(parser)
}

/// Compile a single tree-sitter [`Query`] with a descriptive error on failure.
///
/// Replaces the repeated two-line `Query::new(...).map_err(...)` pattern in
/// every parser that compiles named queries.
pub fn compile_query(language: &Language, pattern: &str, query_name: &str) -> Result<Query> {
    Query::new(language, pattern)
        .map_err(|e| anyhow::anyhow!("Failed to compile {} query: {}", query_name, e))
}

/// Internal: run `query` captures, invoking `f(capture_index, node)` for each.
fn for_each_capture(
    query: &Query,
    root_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    mut f: impl for<'n> FnMut(usize, tree_sitter::Node<'n>),
) {
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root_node, source_bytes);
    while let Some(m) = iter.next() {
        for capture in m.captures {
            f(capture.index as usize, capture.node);
        }
    }
}

/// Generic capture collector: run `query`, apply `extract`, deduplicate, and return sorted.
///
/// `extract(capture_index, node) -> Option<T>` — return `Some(item)` to include it, `None` to skip.
/// Items are deduplicated using `T`'s `Hash + Eq` and sorted with `T`'s `Ord`.
///
/// Building block for [`collect_matches`] and [`collect_named_matches`].
pub fn collect_captures<T, F>(
    query: &Query,
    root_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    mut extract: F,
) -> Vec<T>
where
    T: Eq + Hash + Ord,
    F: for<'n> FnMut(usize, tree_sitter::Node<'n>) -> Option<T>,
{
    let mut seen = HashSet::new();
    for_each_capture(query, root_node, source_bytes, |idx, node| {
        if let Some(item) = extract(idx, node) {
            seen.insert(item);
        }
    });
    let mut results: Vec<T> = seen.into_iter().collect();
    results.sort();
    results
}

/// Collect unique text from all captures of a query, returned as a sorted Vec.
pub fn collect_matches(
    query: &Query,
    root_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Vec<String> {
    collect_captures(query, root_node, source_bytes, |_, node| {
        node.utf8_text(source_bytes).ok().map(|s| s.to_string())
    })
}

/// Collect unique text from captures matching `capture_name`, returned as a sorted Vec.
pub fn collect_named_matches(
    query: &Query,
    capture_name: &str,
    root_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Vec<String> {
    let capture_names = query.capture_names();
    collect_captures(query, root_node, source_bytes, |idx, node| {
        if idx < capture_names.len() && capture_names[idx] == capture_name {
            node.utf8_text(source_bytes).ok().map(|s| s.to_string())
        } else {
            None
        }
    })
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

/// Collect export entries with line ranges from query captures, deduplicated by name.
///
/// Unlike [`collect_captures`], this function deduplicates by symbol name rather than
/// by the full [`ExportEntry`] value — the same name must not appear twice regardless
/// of line position. Use this for all export extraction.
pub fn collect_matches_with_lines(
    query: &Query,
    root_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Vec<ExportEntry> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut results = Vec::new();
    for_each_capture(query, root_node, source_bytes, |_, node| {
        if let Ok(text) = node.utf8_text(source_bytes) {
            let name = text.to_string();
            if seen.insert(name.clone()) {
                let decl = top_level_ancestor(node);
                results.push(ExportEntry::new(
                    name,
                    decl.start_position().row + 1,
                    decl.end_position().row + 1,
                ));
            }
        }
    });
    results.sort_by_key(|e| e.start_line);
    results
}

/// Check whether a node has a modifier child matching `modifier_node_kind` whose
/// whitespace-split text contains any of the supplied `keywords`.
///
/// Two conventions appear across parsers:
/// - `"modifiers"` (Swift, Kotlin): a single aggregate child whose full text is split
///   by whitespace to find individual modifiers (e.g. `"@objc public"`).
/// - `"modifier"` (C#): multiple individual children, each a single modifier word.
///
/// Both work with this helper. For an aggregate `"modifiers"` node the text is split
/// by whitespace; for individual `"modifier"` nodes each single-word text splits to itself.
pub fn has_modifier(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    modifier_node_kind: &str,
    keywords: &[&str],
) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == modifier_node_kind {
            if let Ok(text) = child.utf8_text(source_bytes) {
                if text.split_whitespace().any(|w| keywords.contains(&w)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Push an export entry if `name` has not already been seen.
///
/// Encapsulates the `seen.insert` + `ExportEntry::new` pattern repeated across
/// the builtin parsers.
pub fn push_export(
    exports: &mut Vec<ExportEntry>,
    seen: &mut HashSet<String>,
    name: String,
    start_line: usize,
    end_line: usize,
) {
    if seen.insert(name.clone()) {
        exports.push(ExportEntry::new(name, start_line, end_line));
    }
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
