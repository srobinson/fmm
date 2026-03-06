use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

use super::query_helpers::make_parser;

pub struct DartParser {
    parser: TSParser,
}

impl DartParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_dart_orchard::LANGUAGE.into();
        let parser = make_parser(&language, "Dart")?;
        Ok(Self { parser })
    }

    /// Check if a name starts with underscore (private in Dart).
    fn is_private(name: &str) -> bool {
        name.starts_with('_')
    }

    /// Extract identifier text from a node's `identifier` child.
    fn get_identifier(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    /// Extract type_identifier text from a node.
    fn get_type_identifier(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    /// Extract the variable name from static_final_declaration_list or initialized_identifier_list.
    fn get_var_name_from_decl(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        // These nodes contain child like "appVersion = '1.0.0'"
        // The identifier is the first part before '='
        if let Ok(text) = node.utf8_text(source_bytes) {
            let name = text.split('=').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        None
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut cursor = root_node.walk();
        let children: Vec<tree_sitter::Node> = root_node.children(&mut cursor).collect();

        let mut i = 0;
        while i < children.len() {
            let child = children[i];
            match child.kind() {
                "class_definition" => {
                    if let Some(name) = Self::get_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "mixin_declaration" => {
                    if let Some(name) = Self::get_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "enum_declaration" => {
                    if let Some(name) = Self::get_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "extension_declaration" => {
                    if let Some(name) = Self::get_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "type_alias" => {
                    if let Some(name) = Self::get_type_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "function_signature" => {
                    // Function name is an identifier child of function_signature
                    if let Some(name) = Self::get_identifier(&child, source_bytes) {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            // End line includes the function_body sibling if present
                            let end_row = if i + 1 < children.len()
                                && children[i + 1].kind() == "function_body"
                            {
                                children[i + 1].end_position().row + 1
                            } else {
                                child.end_position().row + 1
                            };
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                end_row,
                            ));
                        }
                    }
                }
                "final_builtin" | "const_builtin" | "inferred_type" => {
                    // Top-level variable: next siblings include type_identifier (optional)
                    // and static_final_declaration_list or initialized_identifier_list
                    let start_line = child.start_position().row + 1;
                    let mut end_line = child.end_position().row + 1;
                    let mut var_name = None;

                    // Look ahead for the declaration list
                    for sibling in children.iter().skip(i + 1).take(3) {
                        match sibling.kind() {
                            "static_final_declaration_list" | "initialized_identifier_list" => {
                                var_name = Self::get_var_name_from_decl(sibling, source_bytes);
                                end_line = sibling.end_position().row + 1;
                                break;
                            }
                            _ => {}
                        }
                    }

                    if let Some(name) = var_name {
                        if !Self::is_private(&name) && seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(name, start_line, end_line));
                        }
                    }
                }
                _ => {}
            }
            i += 1;
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
        let mut dependency_set = HashSet::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if child.kind() == "import_or_export" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    // Extract the path from import statement
                    if let Some(path) = Self::extract_import_path(text) {
                        if path.starts_with("package:") {
                            // package:flutter/material.dart → flutter
                            let pkg = path
                                .strip_prefix("package:")
                                .unwrap_or("")
                                .split('/')
                                .next()
                                .unwrap_or("");
                            if !pkg.is_empty() {
                                import_set.insert(pkg.to_string());
                            }
                        } else if path.starts_with("dart:") {
                            // dart:async → dart:async
                            import_set.insert(path.to_string());
                        } else if path.starts_with('.') {
                            // Relative imports → dependencies
                            dependency_set.insert(path.to_string());
                        } else {
                            import_set.insert(path.to_string());
                        }
                    }
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        let mut dependencies: Vec<String> = dependency_set.into_iter().collect();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }

    /// Extract path from an import statement text like "import 'package:flutter/material.dart';"
    /// Handles both single and double quotes.
    fn extract_import_path(text: &str) -> Option<String> {
        // Find the first quote (single or double)
        let (start, quote) = text
            .find('\'')
            .map(|i| (i, '\''))
            .or_else(|| text.find('"').map(|i| (i, '"')))?;
        let rest = &text[start + 1..];
        let end = rest.find(quote)?;
        Some(rest[..end].to_string())
    }

    fn extract_custom_fields(
        &self,
        root_node: tree_sitter::Node,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let mut mixin_count: u64 = 0;
        let mut extension_count: u64 = 0;
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "mixin_declaration" => mixin_count += 1,
                "extension_declaration" => extension_count += 1,
                _ => {}
            }
        }

        let mut fields = HashMap::new();
        if mixin_count > 0 {
            fields.insert(
                "mixins".to_string(),
                serde_json::Value::Number(mixin_count.into()),
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

impl Parser for DartParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Dart source"))?;

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
        "dart"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_public_function() {
        let mut parser = DartParser::new().unwrap();
        let source = "void hello() {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["hello"]);
    }

    #[test]
    fn exclude_private_underscore() {
        let mut parser = DartParser::new().unwrap();
        let source = "void _privateFunc() {}\nvoid publicFunc() {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["publicFunc"]);
    }

    #[test]
    fn parse_class() {
        let mut parser = DartParser::new().unwrap();
        let source = "class MyClass {}\nclass _Private {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["MyClass"]);
    }

    #[test]
    fn parse_enum() {
        let mut parser = DartParser::new().unwrap();
        let source = "enum Color { red, green, blue }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Color"]);
    }

    #[test]
    fn parse_mixin() {
        let mut parser = DartParser::new().unwrap();
        let source = "mixin Loggable {\n  void log(String msg) {}\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Loggable"]);
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("mixins").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn parse_package_imports() {
        let mut parser = DartParser::new().unwrap();
        let source = "import 'package:flutter/material.dart';\nimport 'dart:async';\nimport './local.dart';\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"flutter".to_string()));
        assert!(result.metadata.imports.contains(&"dart:async".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./local.dart".to_string()));
    }

    #[test]
    fn parse_double_quoted_imports() {
        let mut parser = DartParser::new().unwrap();
        let source =
            "import \"package:provider/provider.dart\";\nimport \"dart:io\";\nimport \"./utils.dart\";\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"provider".to_string()));
        assert!(result.metadata.imports.contains(&"dart:io".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils.dart".to_string()));
    }

    #[test]
    fn parse_typedef() {
        let mut parser = DartParser::new().unwrap();
        let source = "typedef Callback = void Function(String);\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Callback"]);
    }

    #[test]
    fn parse_extension() {
        let mut parser = DartParser::new().unwrap();
        let source = "extension StringExt on String {\n  String upper() => toUpperCase();\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["StringExt"]);
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("extensions").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn exports_sorted_by_line() {
        let mut parser = DartParser::new().unwrap();
        let source = "void zebra() {}\nvoid alpha() {}\nvoid middle() {}\n";
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
