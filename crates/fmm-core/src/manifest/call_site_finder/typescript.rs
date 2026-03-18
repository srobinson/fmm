//! TypeScript/JavaScript call-site verification.

use super::{BareCallSiteResult, CallSiteVerifier};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TSParser, Query, QueryCursor};

pub(super) struct TsCallSiteVerifier;

impl CallSiteVerifier for TsCallSiteVerifier {
    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn method_call_exists(&self, source: &[u8], method_name: &str) -> Option<bool> {
        call_exists_ts(source, method_name)
    }

    fn bare_call_result(&self, source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
        bare_call_result_ts(source, fn_name)
    }
}

/// Check for TypeScript/JS call expression: `something.METHOD_NAME(...)`
fn call_exists_ts(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call_expression
            function: (member_expression
                property: (property_identifier) @prop
                (#eq? @prop "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(super::has_any_match(&query, tree.root_node(), source))
}

/// Analyse a TS/JS source file for bare function calls of `fn_name`.
///
/// Algorithm:
/// 1. Parse imports to find how `fn_name` is imported locally (direct, aliased, namespace, none).
/// 2. For direct/aliased imports, run a call_expression query for the local name.
/// 3. For namespace imports, return `NamespaceCaller` immediately (can't narrow).
/// 4. If not imported at all, return `NotACaller`.
fn bare_call_result_ts(source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();

    // Query A: direct named import `import { fn_name }`
    let q_direct_src = format!(
        r#"(import_statement
            (import_clause
                (named_imports
                    (import_specifier !alias
                        name: (identifier) @name
                        (#eq? @name "{}")))))
        "#,
        fn_name
    );

    // Query B: aliased named import `import { fn_name as alias }` — capture @local is the alias
    let q_aliased_src = format!(
        r#"(import_statement
            (import_clause
                (named_imports
                    (import_specifier
                        name: (identifier) @orig
                        alias: (identifier) @local
                        (#eq? @orig "{}")))))
        "#,
        fn_name
    );

    // Query C: namespace import `import * as ns`
    let q_ns_src = r#"(import_statement
        (import_clause
            (namespace_import (identifier) @ns)))"#;

    let q_direct = Query::new(&lang, &q_direct_src).ok()?;
    let q_aliased = Query::new(&lang, &q_aliased_src).ok()?;
    let q_ns = Query::new(&lang, q_ns_src).ok()?;

    let mut local_names: Vec<String> = Vec::new();
    let mut ns_names: Vec<String> = Vec::new();

    // Direct import — local name is fn_name itself
    {
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_direct, root, source);
        if iter.next().is_some() {
            local_names.push(fn_name.to_string());
        }
    }

    // Aliased import — collect all aliases
    {
        let local_idx = q_aliased
            .capture_index_for_name("local")
            .unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_aliased, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == local_idx
                    && let Ok(text) = cap.node.utf8_text(source)
                {
                    local_names.push(text.to_string());
                }
            }
        }
    }

    // Namespace imports — collect all namespace names
    {
        let ns_idx = q_ns.capture_index_for_name("ns").unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_ns, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == ns_idx
                    && let Ok(text) = cap.node.utf8_text(source)
                {
                    ns_names.push(text.to_string());
                }
            }
        }
    }

    // If namespace import found and no direct/aliased, return NamespaceCaller
    if !ns_names.is_empty() && local_names.is_empty() {
        return Some(BareCallSiteResult::NamespaceCaller(
            ns_names.into_iter().next().unwrap(),
        ));
    }

    // If no direct/aliased import, return NotACaller
    if local_names.is_empty() {
        return Some(BareCallSiteResult::NotACaller);
    }

    // Search for bare call expressions using each local name
    for local_name in &local_names {
        if !super::is_valid_identifier(local_name) {
            continue;
        }
        let q_call_src = format!(
            r#"(call_expression function: (identifier) @callee (#eq? @callee "{}"))"#,
            local_name
        );
        if let Ok(q_call) = Query::new(&lang, &q_call_src)
            && super::has_any_match(&q_call, root, source)
        {
            return Some(BareCallSiteResult::DirectCaller);
        }
    }

    // Imported but never called directly
    Some(BareCallSiteResult::NotACaller)
}
