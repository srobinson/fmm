//! Python private member and top-level function extraction.

use super::{PrivateMember, PrivateMemberExtractor, TopLevelFunction};
use anyhow::Result;
use std::collections::HashMap;

pub(super) struct PyPrivateMemberExtractor;

impl PrivateMemberExtractor for PyPrivateMemberExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn extract_top_level_functions(
        &self,
        source: &[u8],
        exports: &[&str],
    ) -> Result<Vec<TopLevelFunction>> {
        Ok(extract_py_top_level(source, exports).unwrap_or_default())
    }

    fn extract_private_members(
        &self,
        source: &[u8],
        class_names: &[&str],
    ) -> Result<HashMap<String, Vec<PrivateMember>>> {
        Ok(extract_py_private(source, class_names).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Python top-level function extraction (ALP-910)
// ---------------------------------------------------------------------------

fn extract_py_top_level(source: &[u8], exports: &[&str]) -> Option<Vec<TopLevelFunction>> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result = Vec::new();
    let root = tree.root_node();

    for i in 0..root.child_count() {
        let child = match root.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        if child.kind() == "function_definition"
            && let Some(name_node) = child.child_by_field_name("name")
            && let Ok(name) = name_node.utf8_text(source)
            && !exports.contains(&name)
        {
            result.push(TopLevelFunction {
                name: name.to_string(),
                start: child.start_position().row + 1,
                end: child.end_position().row + 1,
            });
        }
    }

    result.sort_by_key(|f| f.start);
    Some(result)
}

// ---------------------------------------------------------------------------
// Python private member extraction
// ---------------------------------------------------------------------------

fn extract_py_private(
    source: &[u8],
    class_names: &[&str],
) -> Option<HashMap<String, Vec<PrivateMember>>> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result: HashMap<String, Vec<PrivateMember>> = HashMap::new();
    walk_py_node(tree.root_node(), source, class_names, &mut result);
    Some(result)
}

fn walk_py_node(
    node: tree_sitter::Node,
    source: &[u8],
    class_names: &[&str],
    result: &mut HashMap<String, Vec<PrivateMember>>,
) {
    if node.kind() == "class_definition"
        && let Some(name_node) = node.child_by_field_name("name")
        && let Ok(name) = name_node.utf8_text(source)
        && class_names.contains(&name)
        && let Some(body) = node.child_by_field_name("body")
    {
        let members = collect_py_private_members(body, source);
        if !members.is_empty() {
            result.insert(name.to_string(), members);
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk_py_node(child, source, class_names, result);
        }
    }
}

fn collect_py_private_members(body: tree_sitter::Node, source: &[u8]) -> Vec<PrivateMember> {
    let mut members = Vec::new();

    for i in 0..body.child_count() {
        let child = match body.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        if child.kind() == "function_definition"
            && let Some(name_node) = child.child_by_field_name("name")
            && let Ok(name) = name_node.utf8_text(source)
            && is_py_private(name)
        {
            members.push(PrivateMember {
                name: name.to_string(),
                start: child.start_position().row + 1,
                end: child.end_position().row + 1,
                is_method: true,
            });
        }
    }

    members.sort_by_key(|m| m.start);
    members
}

/// Python private convention: `_name` or `__name` (single/double prefix),
/// but NOT dunder methods (`__name__`) which are magic, not private.
fn is_py_private(name: &str) -> bool {
    if !name.starts_with('_') {
        return false;
    }
    // Exclude dunder methods (__init__, __repr__, etc.)
    if name.starts_with("__") && name.ends_with("__") {
        return false;
    }
    true
}
