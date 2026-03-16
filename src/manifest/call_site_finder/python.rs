//! Python call-site verification.

use super::CallSiteVerifier;
use tree_sitter::{Parser as TSParser, Query};

pub(super) struct PyCallSiteVerifier;

impl CallSiteVerifier for PyCallSiteVerifier {
    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn method_call_exists(&self, source: &[u8], method_name: &str) -> Option<bool> {
        call_exists_py(source, method_name)
    }
}

/// Check for Python call expression: `something.METHOD_NAME(...)`
fn call_exists_py(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call
            function: (attribute
                attribute: (identifier) @attr
                (#eq? @attr "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(super::has_any_match(&query, tree.root_node(), source))
}
