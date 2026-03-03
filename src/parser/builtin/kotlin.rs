use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

pub struct KotlinParser {
    parser: TSParser,
}

impl KotlinParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_kotlin_ng::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Kotlin language: {}", e))?;
        Ok(Self { parser })
    }

    /// Check if a node has `private` or `internal` modifiers (which exclude from export).
    /// In Kotlin, default visibility is public, so absence of modifiers = exported.
    fn is_private_or_internal(node: &tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    return text
                        .split_whitespace()
                        .any(|w| w == "private" || w == "internal" || w == "protected");
                }
            }
        }
        false
    }

    /// Extract name from `identifier` child.
    fn get_identifier(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    /// Extract name from `variable_declaration` child.
    fn get_variable_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    /// Check if modifiers contain a specific keyword.
    fn has_modifier(node: &tree_sitter::Node, source_bytes: &[u8], modifier: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    return text.split_whitespace().any(|w| w == modifier);
                }
            }
        }
        false
    }

    /// Count companion objects inside class bodies (recursive walk).
    fn count_companion_objects(node: tree_sitter::Node, source_bytes: &[u8]) -> u64 {
        let _ = source_bytes;
        let mut count: u64 = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "companion_object" {
                count += 1;
            }
            // Recurse into class bodies
            if child.kind() == "class_body" {
                count += Self::count_companion_objects(child, source_bytes);
            }
        }
        count
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "class_declaration" => {
                    if !Self::is_private_or_internal(&child, source_bytes) {
                        if let Some(name) = Self::get_identifier(&child, source_bytes) {
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
                "object_declaration" => {
                    if !Self::is_private_or_internal(&child, source_bytes) {
                        if let Some(name) = Self::get_identifier(&child, source_bytes) {
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
                "function_declaration" => {
                    if !Self::is_private_or_internal(&child, source_bytes) {
                        if let Some(name) = Self::get_identifier(&child, source_bytes) {
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
                "property_declaration" => {
                    if !Self::is_private_or_internal(&child, source_bytes) {
                        if let Some(name) = Self::get_variable_name(&child, source_bytes) {
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
                "type_alias" => {
                    if !Self::is_private_or_internal(&child, source_bytes) {
                        if let Some(name) = Self::get_identifier(&child, source_bytes) {
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

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if child.kind() == "import" {
                let mut inner = child.walk();
                for import_child in child.children(&mut inner) {
                    if import_child.kind() == "qualified_identifier" {
                        if let Ok(full_path) = import_child.utf8_text(source_bytes) {
                            // Extract first two segments as the package root
                            // e.g., kotlin.collections.List → kotlin.collections
                            let segments: Vec<&str> = full_path.split('.').collect();
                            let root = if segments.len() >= 2 {
                                format!("{}.{}", segments[0], segments[1])
                            } else {
                                segments[0].to_string()
                            };
                            import_set.insert(root);
                        }
                    }
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        imports.sort();
        let dependencies: Vec<String> = Vec::new();
        (imports, dependencies)
    }

    fn extract_custom_fields(
        &self,
        root_node: tree_sitter::Node,
        source_bytes: &[u8],
    ) -> Option<HashMap<String, serde_json::Value>> {
        let mut data_classes: u64 = 0;
        let mut sealed_classes: u64 = 0;
        let mut companion_objects: u64 = 0;
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if child.kind() == "class_declaration" {
                if Self::has_modifier(&child, source_bytes, "data") {
                    data_classes += 1;
                }
                if Self::has_modifier(&child, source_bytes, "sealed") {
                    sealed_classes += 1;
                }
                // Look for companion objects inside class body
                let mut inner = child.walk();
                for inner_child in child.children(&mut inner) {
                    if inner_child.kind() == "class_body" {
                        companion_objects +=
                            Self::count_companion_objects(inner_child, source_bytes);
                    }
                }
            }
        }

        let mut fields = HashMap::new();
        if data_classes > 0 {
            fields.insert(
                "data_classes".to_string(),
                serde_json::Value::Number(data_classes.into()),
            );
        }
        if sealed_classes > 0 {
            fields.insert(
                "sealed_classes".to_string(),
                serde_json::Value::Number(sealed_classes.into()),
            );
        }
        if companion_objects > 0 {
            fields.insert(
                "companion_objects".to_string(),
                serde_json::Value::Number(companion_objects.into()),
            );
        }

        if fields.is_empty() {
            None
        } else {
            Some(fields)
        }
    }
}

impl Parser for KotlinParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Kotlin source"))?;

        let root_node = tree.root_node();
        let source_bytes = source.as_bytes();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let custom_fields = self.extract_custom_fields(root_node, source_bytes);
        let loc = source.lines().count();

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
            },
            custom_fields,
        })
    }

    fn language_id(&self) -> &'static str {
        "kotlin"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_public_function() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "fun hello(): String = \"hello\"\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["hello"]);
    }

    #[test]
    fn exclude_private_and_internal() {
        let mut parser = KotlinParser::new().unwrap();
        let source =
            "private fun secret() {}\ninternal fun module() {}\nfun visible(): String = \"yes\"\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["visible"]);
    }

    #[test]
    fn parse_class_and_interface() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "class MyClass {}\ninterface MyInterface {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"MyClass".to_string()));
        assert!(names.contains(&"MyInterface".to_string()));
    }

    #[test]
    fn parse_data_class() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "data class User(val name: String)\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["User"]);
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("data_classes").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn parse_object_declaration() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "object Config {\n    val name = \"test\"\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Config"]);
    }

    #[test]
    fn parse_imports_with_package_root() {
        let mut parser = KotlinParser::new().unwrap();
        let source =
            "import kotlin.collections.List\nimport java.util.UUID\nimport org.example.Foo\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .imports
            .contains(&"kotlin.collections".to_string()));
        assert!(result.metadata.imports.contains(&"java.util".to_string()));
        assert!(result.metadata.imports.contains(&"org.example".to_string()));
    }

    #[test]
    fn parse_val_var() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "val MAX = 100\nvar count = 0\nprivate val secret = 42\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"MAX".to_string()));
        assert!(names.contains(&"count".to_string()));
        assert!(!names.contains(&"secret".to_string()));
    }

    #[test]
    fn parse_typealias() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "typealias StringMap = Map<String, String>\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["StringMap"]);
    }

    #[test]
    fn parse_sealed_class() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "sealed class Result {\n    data class Ok(val v: Any) : Result()\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Result"]);
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("sealed_classes").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn parse_enum_class() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "enum class Color { RED, GREEN, BLUE }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["Color"]);
    }

    #[test]
    fn exports_sorted_by_line() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "fun zebra() {}\nfun alpha() {}\nfun middle() {}\n";
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
