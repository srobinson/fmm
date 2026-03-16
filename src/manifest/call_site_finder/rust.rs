//! Rust call-site verification.

use super::CallSiteVerifier;
use tree_sitter::{Parser as TSParser, Query};

pub(super) struct RsCallSiteVerifier;

impl CallSiteVerifier for RsCallSiteVerifier {
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn method_call_exists(&self, source: &[u8], method_name: &str) -> Option<bool> {
        call_exists_rs(source, method_name)
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
