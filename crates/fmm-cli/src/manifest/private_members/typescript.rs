//! TypeScript/JavaScript private member and top-level function extraction.

use super::{PrivateMember, PrivateMemberExtractor, TopLevelFunction};
use anyhow::Result;
use std::collections::HashMap;

pub(super) struct TsPrivateMemberExtractor;

impl PrivateMemberExtractor for TsPrivateMemberExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn extract_top_level_functions(
        &self,
        source: &[u8],
        exports: &[&str],
    ) -> Result<Vec<TopLevelFunction>> {
        Ok(extract_ts_top_level(source, exports).unwrap_or_default())
    }

    fn extract_private_members(
        &self,
        source: &[u8],
        class_names: &[&str],
    ) -> Result<HashMap<String, Vec<PrivateMember>>> {
        Ok(extract_ts_private(source, class_names).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// TypeScript / JS top-level function extraction (ALP-910)
// ---------------------------------------------------------------------------

fn extract_ts_top_level(source: &[u8], exports: &[&str]) -> Option<Vec<TopLevelFunction>> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
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

        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        if !exports.contains(&name) {
                            result.push(TopLevelFunction {
                                name: name.to_string(),
                                start: child.start_position().row + 1,
                                end: child.end_position().row + 1,
                            });
                        }
                    }
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                // const/let/var foo = () => {} or = function() {}
                for j in 0..child.child_count() {
                    let decl = match child.child(j as u32) {
                        Some(c) => c,
                        None => continue,
                    };
                    if decl.kind() != "variable_declarator" {
                        continue;
                    }
                    let name_node = match decl.child_by_field_name("name") {
                        Some(n) => n,
                        None => continue,
                    };
                    let value_node = match decl.child_by_field_name("value") {
                        Some(v) => v,
                        None => continue,
                    };
                    if matches!(
                        value_node.kind(),
                        "arrow_function" | "function_expression" | "generator_function"
                    ) {
                        if let Ok(name) = name_node.utf8_text(source) {
                            if !exports.contains(&name) {
                                result.push(TopLevelFunction {
                                    name: name.to_string(),
                                    start: child.start_position().row + 1,
                                    end: child.end_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
            "class_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        if !exports.contains(&name) {
                            result.push(TopLevelFunction {
                                name: name.to_string(),
                                start: child.start_position().row + 1,
                                end: child.end_position().row + 1,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    result.sort_by_key(|f| f.start);
    Some(result)
}

// ---------------------------------------------------------------------------
// TypeScript / JS private member extraction
// ---------------------------------------------------------------------------

fn extract_ts_private(
    source: &[u8],
    class_names: &[&str],
) -> Option<HashMap<String, Vec<PrivateMember>>> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result: HashMap<String, Vec<PrivateMember>> = HashMap::new();
    walk_ts_node(tree.root_node(), source, class_names, &mut result);
    Some(result)
}

fn walk_ts_node(
    node: tree_sitter::Node,
    source: &[u8],
    class_names: &[&str],
    result: &mut HashMap<String, Vec<PrivateMember>>,
) {
    if node.kind() == "class_declaration" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(source) {
                if class_names.contains(&name) {
                    if let Some(body) = node.child_by_field_name("body") {
                        let members = collect_ts_private_members(body, source);
                        if !members.is_empty() {
                            result.insert(name.to_string(), members);
                        }
                    }
                }
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk_ts_node(child, source, class_names, result);
        }
    }
}

fn collect_ts_private_members(body: tree_sitter::Node, source: &[u8]) -> Vec<PrivateMember> {
    let mut members = Vec::new();

    for i in 0..body.child_count() {
        let child = match body.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "method_definition" => {
                if let Some(m) = ts_private_method(child, source) {
                    members.push(m);
                }
            }
            // Property declarations: `private pool: Pool;`
            "public_field_definition" => {
                if let Some(m) = ts_private_field(child, source) {
                    members.push(m);
                }
            }
            _ => {}
        }
    }

    members.sort_by_key(|m| m.start);
    members
}

/// Extract a private/protected method_definition. Returns None when public.
///
/// Handles two private-method syntaxes:
/// - TypeScript `private`/`protected` keyword: `accessibility_modifier` child present
/// - ECMAScript `#method`: name child kind is `private_property_identifier`
fn ts_private_method(node: tree_sitter::Node, source: &[u8]) -> Option<PrivateMember> {
    let name_node = node.child_by_field_name("name")?;
    let name_kind = name_node.kind();
    let name = name_node.utf8_text(source).ok()?.to_string();

    // Skip computed names like [Symbol.iterator]
    if name.starts_with('[') {
        return None;
    }

    // ECMAScript #method — private_property_identifier is the name
    if name_kind == "private_property_identifier" {
        return Some(PrivateMember {
            name,
            start: node.start_position().row + 1,
            end: node.end_position().row + 1,
            is_method: true,
        });
    }

    // TypeScript `private`/`protected` keyword
    let has_modifier = (0..node.child_count()).any(|i| {
        node.child(i as u32)
            .filter(|c| c.kind() == "accessibility_modifier")
            .and_then(|c| c.utf8_text(source).ok())
            .map(|t| t == "private" || t == "protected")
            .unwrap_or(false)
    });
    if !has_modifier {
        return None;
    }

    Some(PrivateMember {
        name,
        start: node.start_position().row + 1,
        end: node.end_position().row + 1,
        is_method: true,
    })
}

/// Extract a private/protected field declaration. Returns None when public.
///
/// In tree-sitter-typescript, all field declarations use `public_field_definition`
/// regardless of access modifier. Two private-field syntaxes are handled:
/// - TypeScript `private`/`protected` keyword: `accessibility_modifier` child present
/// - ECMAScript `#field`: name child kind is `private_property_identifier`
fn ts_private_field(node: tree_sitter::Node, source: &[u8]) -> Option<PrivateMember> {
    let name_node = node.child_by_field_name("name")?;
    let name_kind = name_node.kind();
    let name = name_node.utf8_text(source).ok()?.to_string();

    // Skip computed property names like [Symbol.hasInstance]
    if name.starts_with('[') {
        return None;
    }

    // ECMAScript #field — private_property_identifier is the name child
    if name_kind == "private_property_identifier" {
        return Some(PrivateMember {
            name,
            start: node.start_position().row + 1,
            end: node.end_position().row + 1,
            is_method: false,
        });
    }

    // TypeScript `private`/`protected` keyword
    let has_modifier = (0..node.child_count()).any(|i| {
        node.child(i as u32)
            .filter(|c| c.kind() == "accessibility_modifier")
            .and_then(|c| c.utf8_text(source).ok())
            .map(|t| t == "private" || t == "protected")
            .unwrap_or(false)
    });
    if !has_modifier {
        return None;
    }

    Some(PrivateMember {
        name,
        start: node.start_position().row + 1,
        end: node.end_position().row + 1,
        is_method: false,
    })
}
