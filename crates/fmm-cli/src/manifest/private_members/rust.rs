//! Rust private member and top-level function extraction.

use super::{PrivateMember, PrivateMemberExtractor, TopLevelFunction};
use anyhow::Result;
use std::collections::HashMap;

pub(super) struct RsPrivateMemberExtractor;

impl PrivateMemberExtractor for RsPrivateMemberExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn extract_top_level_functions(
        &self,
        source: &[u8],
        exports: &[&str],
    ) -> Result<Vec<TopLevelFunction>> {
        Ok(extract_rs_top_level(source, exports).unwrap_or_default())
    }

    fn extract_private_members(
        &self,
        source: &[u8],
        class_names: &[&str],
    ) -> Result<HashMap<String, Vec<PrivateMember>>> {
        Ok(extract_rs_private(source, class_names).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Rust top-level function extraction
// ---------------------------------------------------------------------------

/// Extract non-pub top-level functions from a Rust source file.
///
/// A function is considered non-exported (private) when it has no
/// `visibility_modifier` child node. Functions named in `exports` are
/// excluded to avoid duplicating symbols already shown in the main outline.
fn extract_rs_top_level(source: &[u8], exports: &[&str]) -> Option<Vec<TopLevelFunction>> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
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

        if child.kind() == "function_item" && !has_visibility(child) {
            if let Some(name) = fn_name(child, source) {
                if !exports.contains(&name.as_str()) {
                    result.push(TopLevelFunction {
                        name,
                        start: child.start_position().row + 1,
                        end: child.end_position().row + 1,
                    });
                }
            }
        }
    }

    result.sort_by_key(|f| f.start);
    Some(result)
}

// ---------------------------------------------------------------------------
// Rust private member extraction (impl blocks)
// ---------------------------------------------------------------------------

/// Extract non-pub methods from `impl` blocks whose type name matches
/// one of `class_names`.
///
/// In Rust, "class" maps to the type in an `impl TypeName { ... }` block.
/// Private methods are `fn` items inside the impl body that lack a
/// `visibility_modifier` child.
fn extract_rs_private(
    source: &[u8],
    class_names: &[&str],
) -> Option<HashMap<String, Vec<PrivateMember>>> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result: HashMap<String, Vec<PrivateMember>> = HashMap::new();
    let root = tree.root_node();

    for i in 0..root.child_count() {
        let child = match root.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        if child.kind() != "impl_item" {
            continue;
        }

        // Extract the type name from the impl block.
        // Handles both `impl Foo { ... }` and `impl Trait for Foo { ... }`.
        let type_name = impl_type_name(child, source);
        let Some(ref type_name) = type_name else {
            continue;
        };

        if !class_names.contains(&type_name.as_str()) {
            continue;
        }

        if let Some(body) = child.child_by_field_name("body") {
            let members = collect_private_methods(body, source);
            if !members.is_empty() {
                result.entry(type_name.clone()).or_default().extend(members);
            }
        }
    }

    // Sort each type's members by start line
    for members in result.values_mut() {
        members.sort_by_key(|m| m.start);
    }

    Some(result)
}

/// Collect non-pub function_item nodes from an impl body (declaration_list).
fn collect_private_methods(body: tree_sitter::Node, source: &[u8]) -> Vec<PrivateMember> {
    let mut members = Vec::new();

    for i in 0..body.child_count() {
        let child = match body.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        if child.kind() == "function_item" && !has_visibility(child) {
            if let Some(name) = fn_name(child, source) {
                members.push(PrivateMember {
                    name,
                    start: child.start_position().row + 1,
                    end: child.end_position().row + 1,
                    is_method: true,
                });
            }
        }
    }

    members
}

/// Extract the type name from an `impl_item` node.
///
/// For `impl Foo { ... }`, returns "Foo".
/// For `impl Trait for Foo { ... }`, returns "Foo" (the `type` field).
/// For `impl Foo<T> { ... }`, returns "Foo" (just the identifier).
fn impl_type_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    // tree-sitter-rust uses `type` field for the implemented type
    let type_node = node.child_by_field_name("type")?;
    match type_node.kind() {
        "type_identifier" => type_node.utf8_text(source).ok().map(|s| s.to_string()),
        "generic_type" => {
            // `Foo<T>` -> extract just "Foo"
            type_node
                .child_by_field_name("type")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.to_string())
        }
        _ => type_node.utf8_text(source).ok().map(|s| s.to_string()),
    }
}

/// Check if a node has a `visibility_modifier` child (pub, pub(crate), etc.).
fn has_visibility(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
    }
    false
}

/// Extract the function name from a `function_item` node.
fn fn_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}
