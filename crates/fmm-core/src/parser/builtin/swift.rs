use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

use super::query_helpers::{extract_child_text, has_modifier, make_parser, push_export};

pub struct SwiftParser {
    parser: TSParser,
}

impl SwiftParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_swift::LANGUAGE.into();
        let parser = make_parser(&language, "Swift")?;
        Ok(Self { parser })
    }

    /// Get the declaration keyword for a class_declaration (class, struct, enum, extension).
    fn get_class_keyword(node: &tree_sitter::Node) -> Option<&'static str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class" => return Some("class"),
                "struct" => return Some("struct"),
                "enum" => return Some("enum"),
                "extension" => return Some("extension"),
                _ => {}
            }
        }
        None
    }

    /// Extract property name from a property_declaration (pattern child).
    fn get_property_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pattern" {
                // pattern may contain simple_identifier or just be the name
                let mut inner = child.walk();
                for inner_child in child.children(&mut inner) {
                    if inner_child.kind() == "simple_identifier" {
                        return inner_child
                            .utf8_text(source_bytes)
                            .ok()
                            .map(|s| s.to_string());
                    }
                }
                // If no simple_identifier child, the pattern itself is the name
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    /// Extract methods and properties from an extension's class_body.
    fn extract_extension_members(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "class_body" {
                let mut body_cursor = child.walk();
                for body_child in child.children(&mut body_cursor) {
                    match body_child.kind() {
                        "function_declaration" => {
                            if let Some(name) =
                                extract_child_text(&body_child, source_bytes, "simple_identifier")
                            {
                                push_export(
                                    exports,
                                    seen,
                                    name,
                                    body_child.start_position().row + 1,
                                    body_child.end_position().row + 1,
                                );
                            }
                        }
                        "property_declaration" => {
                            if let Some(name) = Self::get_property_name(&body_child, source_bytes) {
                                push_export(
                                    exports,
                                    seen,
                                    name,
                                    body_child.start_position().row + 1,
                                    body_child.end_position().row + 1,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "class_declaration" => {
                    let is_public =
                        has_modifier(&child, source_bytes, "modifiers", &["public", "open"]);
                    let keyword = Self::get_class_keyword(&child);

                    match keyword {
                        Some("extension")
                            // Public extensions: export their methods
                            if is_public => {
                                Self::extract_extension_members(
                                    &child,
                                    source_bytes,
                                    &mut seen,
                                    &mut exports,
                                );
                            }
                        Some("class") | Some("struct") | Some("enum") => {
                            if is_public
                                && let Some(name) =
                                    extract_child_text(&child, source_bytes, "type_identifier")
                            {
                                push_export(
                                    &mut exports,
                                    &mut seen,
                                    name,
                                    child.start_position().row + 1,
                                    child.end_position().row + 1,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                "protocol_declaration" => {
                    if has_modifier(&child, source_bytes, "modifiers", &["public", "open"])
                        && let Some(name) =
                            extract_child_text(&child, source_bytes, "type_identifier")
                    {
                        push_export(
                            &mut exports,
                            &mut seen,
                            name,
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                        );
                    }
                }
                "function_declaration" => {
                    if has_modifier(&child, source_bytes, "modifiers", &["public", "open"])
                        && let Some(name) =
                            extract_child_text(&child, source_bytes, "simple_identifier")
                    {
                        push_export(
                            &mut exports,
                            &mut seen,
                            name,
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                        );
                    }
                }
                "property_declaration" => {
                    if has_modifier(&child, source_bytes, "modifiers", &["public", "open"])
                        && let Some(name) = Self::get_property_name(&child, source_bytes)
                    {
                        push_export(
                            &mut exports,
                            &mut seen,
                            name,
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                        );
                    }
                }
                "typealias_declaration" => {
                    if has_modifier(&child, source_bytes, "modifiers", &["public", "open"])
                        && let Some(name) =
                            extract_child_text(&child, source_bytes, "type_identifier")
                    {
                        push_export(
                            &mut exports,
                            &mut seen,
                            name,
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                        );
                    }
                }
                _ => {}
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if child.kind() == "import_declaration" {
                let mut inner = child.walk();
                for import_child in child.children(&mut inner) {
                    if import_child.kind() == "identifier"
                        && let Ok(name) = import_child.utf8_text(source_bytes)
                    {
                        import_set.insert(name.to_string());
                    }
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        imports.sort();
        // Swift doesn't have relative file imports
        let dependencies: Vec<String> = Vec::new();
        (imports, dependencies)
    }

    fn extract_custom_fields(
        &self,
        root_node: tree_sitter::Node,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let mut protocol_count: u64 = 0;
        let mut extension_count: u64 = 0;
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "protocol_declaration" => {
                    protocol_count += 1;
                }
                "class_declaration" if Self::get_class_keyword(&child) == Some("extension") => {
                    extension_count += 1;
                }
                _ => {}
            }
        }

        let mut fields = HashMap::new();
        if protocol_count > 0 {
            fields.insert(
                "protocols".to_string(),
                serde_json::Value::Number(protocol_count.into()),
            );
        }
        if extension_count > 0 {
            fields.insert(
                "extensions".to_string(),
                serde_json::Value::Number(extension_count.into()),
            );
        }

        if fields.is_empty() {
            None
        } else {
            Some(fields)
        }
    }
}

impl Parser for SwiftParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Swift source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let custom_fields = self.extract_custom_fields(root_node);
        let loc = source.lines().count();

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
                ..Default::default()
            },
            custom_fields,
        })
    }

    fn language_id(&self) -> &'static str {
        "swift"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "swift",
        extensions: &["swift"],
        reexport_filenames: &[],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &[],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_public_function() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public func hello() -> String { return \"hello\" }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["hello"]);
    }

    #[test]
    fn parse_open_class() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "open class Base {\n    open func setup() {}\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Base".to_string()));
    }

    #[test]
    fn exclude_private_and_internal() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "private func secret() {}\ninternal func helper() {}\nfileprivate func local() {}\nfunc defaultAccess() {}\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn parse_imports() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "import Foundation\nimport UIKit\n@testable import MyModule\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"Foundation".to_string()));
        assert!(result.metadata.imports.contains(&"UIKit".to_string()));
        assert!(result.metadata.imports.contains(&"MyModule".to_string()));
        assert!(result.metadata.dependencies.is_empty());
    }

    #[test]
    fn parse_public_struct_and_enum() {
        let mut parser = SwiftParser::new().unwrap();
        let source =
            "public struct Point { public var x: Double }\npublic enum Color { case red, blue }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Point".to_string()));
        assert!(names.contains(&"Color".to_string()));
    }

    #[test]
    fn parse_protocol() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public protocol Drawable {\n    func draw()\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Drawable"]);
    }

    #[test]
    fn parse_public_let_var() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public let MAX = 100\npublic var count = 0\nprivate let secret = 42\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"MAX".to_string()));
        assert!(names.contains(&"count".to_string()));
        assert!(!names.contains(&"secret".to_string()));
    }

    #[test]
    fn parse_typealias() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public typealias StringMap = [String: String]\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["StringMap"]);
    }

    #[test]
    fn parse_public_extension_methods() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public extension String {\n    func trimmed() -> String { return \"\" }\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"trimmed".to_string()));
    }

    #[test]
    fn non_public_extension_excluded() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "extension Int {\n    func doubled() -> Int { return self * 2 }\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn custom_fields_protocols_and_extensions() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public protocol A {}\npublic protocol B {}\npublic extension String {}\nextension Int {}\n";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("protocols").unwrap().as_u64().unwrap(), 2);
        assert_eq!(fields.get("extensions").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn exports_sorted_by_line() {
        let mut parser = SwiftParser::new().unwrap();
        let source = "public func zebra() {}\npublic func alpha() {}\npublic func middle() {}\n";
        let result = parser.parse(source).unwrap();
        let lines: Vec<usize> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.start_line)
            .collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
    }
}
