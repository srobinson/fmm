use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

use super::query_helpers::make_parser;

pub struct ZigParser {
    parser: TSParser,
}

impl ZigParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_zig::LANGUAGE.into();
        let parser = make_parser(&language, "Zig")?;
        Ok(Self { parser })
    }

    /// Check if a top-level declaration node has a `pub` keyword as its first child.
    fn is_pub(node: &tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pub" {
                return true;
            }
            // pub is always the first keyword — stop after first non-whitespace token
            if child.is_named() {
                break;
            }
        }
        // Also check via text for robustness
        node.utf8_text(source_bytes)
            .map(|t| t.starts_with("pub "))
            .unwrap_or(false)
    }

    /// Extract the identifier name from a function_declaration or variable_declaration.
    fn extract_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" => {
                    if Self::is_pub(&child, source_bytes) {
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            if seen.insert(name.clone()) {
                                exports.push(ExportEntry::new(
                                    name,
                                    child.start_position().row + 1,
                                    child.end_position().row + 1,
                                ));
                            }
                        }
                    }
                }
                "variable_declaration" => {
                    if Self::is_pub(&child, source_bytes) {
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            if seen.insert(name.clone()) {
                                exports.push(ExportEntry::new(
                                    name,
                                    child.start_position().row + 1,
                                    child.end_position().row + 1,
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    /// Extract @import calls from the entire tree.
    /// Relative paths (starting with `.`) → dependencies; everything else → imports.
    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut dependency_set = HashSet::new();

        Self::walk_imports(
            root_node,
            source_bytes,
            &mut import_set,
            &mut dependency_set,
        );

        let mut imports: Vec<String> = import_set.into_iter().collect();
        let mut dependencies: Vec<String> = dependency_set.into_iter().collect();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }

    fn walk_imports(
        node: tree_sitter::Node,
        source_bytes: &[u8],
        imports: &mut HashSet<String>,
        dependencies: &mut HashSet<String>,
    ) {
        if node.kind() == "builtin_function" {
            // Check if this is @import
            let mut cursor = node.walk();
            let mut is_import = false;
            for child in node.children(&mut cursor) {
                if child.kind() == "builtin_identifier" {
                    if let Ok(text) = child.utf8_text(source_bytes) {
                        if text == "@import" {
                            is_import = true;
                        }
                    }
                }
                if is_import && child.kind() == "arguments" {
                    // Find the string content inside the arguments
                    let mut arg_cursor = child.walk();
                    for arg_child in child.children(&mut arg_cursor) {
                        if arg_child.kind() == "string" {
                            // Find string_content child
                            let mut str_cursor = arg_child.walk();
                            for str_child in arg_child.children(&mut str_cursor) {
                                if str_child.kind() == "string_content" {
                                    if let Ok(path) = str_child.utf8_text(source_bytes) {
                                        if !path.is_empty() {
                                            if path.starts_with('.') {
                                                dependencies.insert(path.to_string());
                                            } else {
                                                imports.insert(path.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return; // Don't recurse into builtin_function children again
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_imports(child, source_bytes, imports, dependencies);
        }
    }

    /// Count comptime and test declarations at root level.
    fn count_special_blocks(root_node: tree_sitter::Node) -> (usize, usize) {
        let mut comptime_count = 0;
        let mut test_count = 0;
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "comptime_declaration" => comptime_count += 1,
                "test_declaration" => test_count += 1,
                _ => {}
            }
        }

        (comptime_count, test_count)
    }
}

impl Parser for ZigParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Zig source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let loc = source.lines().count();
        let (comptime_blocks, test_blocks) = Self::count_special_blocks(root_node);

        let mut custom_fields = HashMap::new();
        if comptime_blocks > 0 {
            custom_fields.insert(
                "comptime_blocks".to_string(),
                serde_json::Value::Number(comptime_blocks.into()),
            );
        }
        if test_blocks > 0 {
            custom_fields.insert(
                "test_blocks".to_string(),
                serde_json::Value::Number(test_blocks.into()),
            );
        }

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
                ..Default::default()
            },
            custom_fields: if custom_fields.is_empty() {
                None
            } else {
                Some(custom_fields)
            },
        })
    }

    fn language_id(&self) -> &'static str {
        "zig"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zig_pub_functions() {
        let mut parser = ZigParser::new().unwrap();
        let source = "pub fn exported() void {}\nfn private() void {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"exported".to_string()));
        assert!(!names.contains(&"private".to_string()));
    }

    #[test]
    fn parse_zig_pub_const_and_var() {
        let mut parser = ZigParser::new().unwrap();
        let source =
            "pub const MAX: u32 = 100;\npub var state: bool = false;\nconst internal: u32 = 0;\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"MAX".to_string()));
        assert!(names.contains(&"state".to_string()));
        assert!(!names.contains(&"internal".to_string()));
    }

    #[test]
    fn parse_zig_imports() {
        let mut parser = ZigParser::new().unwrap();
        let source = r#"const std = @import("std");
const builtin = @import("builtin");
const utils = @import("./utils.zig");
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"std".to_string()));
        assert!(result.metadata.imports.contains(&"builtin".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils.zig".to_string()));
    }

    #[test]
    fn parse_zig_struct_type() {
        let mut parser = ZigParser::new().unwrap();
        let source =
            "pub const Config = struct { x: u32, };\nconst Internal = struct { y: u32, };\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Config".to_string()));
        assert!(!names.contains(&"Internal".to_string()));
    }

    #[test]
    fn parse_zig_comptime_and_test_blocks() {
        let mut parser = ZigParser::new().unwrap();
        let source = r#"comptime { _ = 0; }
test "foo" { _ = 0; }
test "bar" { _ = 0; }
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("comptime_blocks").unwrap().as_u64().unwrap(), 1);
        assert_eq!(fields.get("test_blocks").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn parse_zig_empty() {
        let mut parser = ZigParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}

impl crate::parser::LanguageDescriptor for ZigParser {
    fn language_id(&self) -> &'static str {
        "zig"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }
}
