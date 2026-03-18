//! Rust call-site verification.

use super::{BareCallSiteResult, CallSiteVerifier};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TSParser, Query, QueryCursor};

pub(super) struct RsCallSiteVerifier;

impl CallSiteVerifier for RsCallSiteVerifier {
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn method_call_exists(&self, source: &[u8], method_name: &str) -> Option<bool> {
        call_exists_rs(source, method_name)
    }

    fn bare_call_result(&self, source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
        bare_call_result_rs(source, fn_name)
    }
}

/// Check for Rust method call: `something.METHOD_NAME(...)`
fn call_exists_rs(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call_expression
            function: (field_expression
                field: (field_identifier) @field
                (#eq? @field "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(super::has_any_match(&query, tree.root_node(), source))
}

/// Analyse a Rust source file for bare function calls of `fn_name`.
///
/// Algorithm:
/// 1. Check if `fn_name` is imported via a named `use` declaration (direct or aliased).
/// 2. For direct/aliased imports, check for a `call_expression` with the local name.
/// 3. For glob imports (`use path::*`), return `NamespaceCaller` (can't narrow).
/// 4. Check for bare `fn_name()` calls even without use (same-module functions).
/// 5. If not imported and not called, return `NotACaller`.
fn bare_call_result_rs(source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();

    // Collect local names for fn_name from use declarations, and glob import paths.
    let (local_names, glob_paths) = collect_rust_imports(root, source, fn_name);

    // Check for bare call expression: `fn_name(...)` or `alias(...)`
    let names_to_check = if local_names.is_empty() {
        // No explicit import found. The function could be defined in the same module,
        // so check for a bare call with the original name.
        vec![fn_name.to_string()]
    } else {
        local_names
    };

    for local_name in &names_to_check {
        if !super::is_valid_identifier(local_name) {
            continue;
        }
        let q_call_src = format!(
            r#"(call_expression function: (identifier) @callee (#eq? @callee "{local_name}"))"#,
        );
        if let Ok(q_call) = Query::new(&lang, &q_call_src) {
            if super::has_any_match(&q_call, root, source) {
                return Some(BareCallSiteResult::DirectCaller);
            }
        }
    }

    // If glob imports exist and no direct call found, the function could be called
    // through a qualified path or pattern we can't easily detect.
    if let Some(path) = glob_paths.into_iter().next() {
        return Some(BareCallSiteResult::NamespaceCaller(path));
    }

    Some(BareCallSiteResult::NotACaller)
}

/// Walk top-level `use_declaration` nodes to find imports of `fn_name`.
///
/// Returns `(local_names, glob_paths)`:
/// - `local_names`: local identifiers that resolve to fn_name (original or alias).
/// - `glob_paths`: paths from `use path::*` that could bring fn_name into scope.
fn collect_rust_imports(
    root: tree_sitter::Node,
    source: &[u8],
    fn_name: &str,
) -> (Vec<String>, Vec<String>) {
    let mut local_names: Vec<String> = Vec::new();
    let mut glob_paths: Vec<String> = Vec::new();

    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

    // Query for direct named import: `use path::fn_name;`
    // This matches scoped_identifier where the name field equals fn_name.
    let q_direct_src = format!(
        r#"(use_declaration
            (scoped_identifier name: (identifier) @name (#eq? @name "{fn_name}")))"#,
    );

    // Query for aliased import: `use path::fn_name as alias;`
    let q_aliased_src = format!(
        r#"(use_declaration
            (use_as_clause
                path: (scoped_identifier name: (identifier) @orig (#eq? @orig "{fn_name}"))
                alias: (identifier) @alias))"#,
    );

    // Query for imports inside use_list: `use path::{fn_name, ...}`
    let q_in_list_src = format!(
        r#"(use_declaration
            (scoped_use_list
                list: (use_list
                    (identifier) @name (#eq? @name "{fn_name}"))))"#,
    );

    // Query for aliased imports inside use_list: `use path::{fn_name as alias, ...}`
    let q_aliased_list_src = format!(
        r#"(use_declaration
            (scoped_use_list
                list: (use_list
                    (use_as_clause
                        path: (identifier) @orig (#eq? @orig "{fn_name}")
                        alias: (identifier) @alias))))"#,
    );

    // Query for glob import: `use path::*`
    let q_glob_src = "(use_declaration (use_wildcard) @wc)";

    // Direct imports
    if let Ok(q) = Query::new(&lang, &q_direct_src) {
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q, root, source);
        if iter.next().is_some() {
            local_names.push(fn_name.to_string());
        }
    }

    // Aliased imports
    if let Ok(q) = Query::new(&lang, &q_aliased_src) {
        let alias_idx = q.capture_index_for_name("alias").unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == alias_idx {
                    if let Ok(text) = cap.node.utf8_text(source) {
                        local_names.push(text.to_string());
                    }
                }
            }
        }
    }

    // Imports inside use_list (direct)
    if let Ok(q) = Query::new(&lang, &q_in_list_src) {
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q, root, source);
        if iter.next().is_some() && !local_names.contains(&fn_name.to_string()) {
            local_names.push(fn_name.to_string());
        }
    }

    // Imports inside use_list (aliased)
    if let Ok(q) = Query::new(&lang, &q_aliased_list_src) {
        let alias_idx = q.capture_index_for_name("alias").unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == alias_idx {
                    if let Ok(text) = cap.node.utf8_text(source) {
                        if !local_names.contains(&text.to_string()) {
                            local_names.push(text.to_string());
                        }
                    }
                }
            }
        }
    }

    // Glob imports
    if let Ok(q) = Query::new(&lang, q_glob_src) {
        let wc_idx = q.capture_index_for_name("wc").unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == wc_idx {
                    if let Ok(text) = cap.node.utf8_text(source) {
                        let path = text.strip_suffix("::*").unwrap_or(text);
                        glob_paths.push(path.to_string());
                    }
                }
            }
        }
    }

    (local_names, glob_paths)
}
