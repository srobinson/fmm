use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

use super::query_helpers::{extract_child_text, has_modifier, make_parser, push_export};

pub struct KotlinParser {
    parser: TSParser,
}

impl KotlinParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_kotlin_ng::LANGUAGE.into();
        let parser = make_parser(&language, "Kotlin")?;
        Ok(Self { parser })
    }

    /// Count companion objects inside class bodies (recursive walk).
    fn count_companion_objects(node: tree_sitter::Node) -> u64 {
        let mut count: u64 = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "companion_object" {
                count += 1;
            }
            // Recurse into class bodies
            if child.kind() == "class_body" {
                count += Self::count_companion_objects(child);
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
                    if !has_modifier(
                        &child,
                        source_bytes,
                        "modifiers",
                        &["private", "internal", "protected"],
                    ) && let Some(class_name) =
                        extract_child_text(&child, source_bytes, "identifier")
                    {
                        push_export(
                            &mut exports,
                            &mut seen,
                            class_name.clone(),
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                        );
                        // ALP-771: extract public methods from the class body
                        let mut class_cursor = child.walk();
                        for class_child in child.children(&mut class_cursor) {
                            if class_child.kind() == "class_body" {
                                let mut body_cursor = class_child.walk();
                                for body_child in class_child.children(&mut body_cursor) {
                                    if body_child.kind() == "function_declaration"
                                        && !has_modifier(
                                            &body_child,
                                            source_bytes,
                                            "modifiers",
                                            &["private", "internal", "protected"],
                                        )
                                        && let Some(method_name) = extract_child_text(
                                            &body_child,
                                            source_bytes,
                                            "identifier",
                                        )
                                    {
                                        let key = format!("{}.{}", class_name, method_name);
                                        if seen.insert(key) {
                                            exports.push(ExportEntry::method(
                                                method_name,
                                                body_child.start_position().row + 1,
                                                body_child.end_position().row + 1,
                                                class_name.clone(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "object_declaration" => {
                    if !has_modifier(
                        &child,
                        source_bytes,
                        "modifiers",
                        &["private", "internal", "protected"],
                    ) && let Some(name) = extract_child_text(&child, source_bytes, "identifier")
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
                    if !has_modifier(
                        &child,
                        source_bytes,
                        "modifiers",
                        &["private", "internal", "protected"],
                    ) && let Some(name) = extract_child_text(&child, source_bytes, "identifier")
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
                    if !has_modifier(
                        &child,
                        source_bytes,
                        "modifiers",
                        &["private", "internal", "protected"],
                    ) && let Some(name) =
                        extract_child_text(&child, source_bytes, "variable_declaration")
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
                "type_alias" => {
                    if !has_modifier(
                        &child,
                        source_bytes,
                        "modifiers",
                        &["private", "internal", "protected"],
                    ) && let Some(name) = extract_child_text(&child, source_bytes, "identifier")
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
            if child.kind() == "import" {
                let mut inner = child.walk();
                for import_child in child.children(&mut inner) {
                    if import_child.kind() == "qualified_identifier"
                        && let Ok(full_path) = import_child.utf8_text(source_bytes)
                    {
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
                if has_modifier(&child, source_bytes, "modifiers", &["data"]) {
                    data_classes += 1;
                }
                if has_modifier(&child, source_bytes, "modifiers", &["sealed"]) {
                    sealed_classes += 1;
                }
                // Look for companion objects inside class body
                let mut inner = child.walk();
                for inner_child in child.children(&mut inner) {
                    if inner_child.kind() == "class_body" {
                        companion_objects += Self::count_companion_objects(inner_child);
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
                ..Default::default()
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

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "kotlin",
        extensions: &["kt", "kts"],
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
        assert!(
            result
                .metadata
                .imports
                .contains(&"kotlin.collections".to_string())
        );
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

    // ALP-771: Kotlin method extraction tests

    fn get_method<'a>(
        exports: &'a [ExportEntry],
        class: &str,
        method: &str,
    ) -> Option<&'a ExportEntry> {
        exports
            .iter()
            .find(|e| e.parent_class.as_deref() == Some(class) && e.name == method)
    }

    #[test]
    fn kotlin_class_methods_extracted_with_parent_class() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "class Greeter {\n    fun greet(): String = \"hello\"\n    fun farewell(): String = \"bye\"\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Greeter", "greet").is_some(),
            "Greeter.greet should be indexed"
        );
        assert!(
            get_method(&result.metadata.exports, "Greeter", "farewell").is_some(),
            "Greeter.farewell should be indexed"
        );
    }

    #[test]
    fn kotlin_private_class_method_excluded() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "class Foo {\n    fun public_method() {}\n    private fun secret() {}\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "public_method").is_some(),
            "public_method should be indexed"
        );
        assert!(
            get_method(&result.metadata.exports, "Foo", "secret").is_none(),
            "private secret() should NOT be indexed"
        );
    }

    #[test]
    fn kotlin_method_not_in_flat_export_names() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "class Foo {\n    fun bar() {}\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            !result.metadata.export_names().contains(&"bar".to_string()),
            "bar should NOT appear in flat export_names()"
        );
        assert!(
            result.metadata.export_names().contains(&"Foo".to_string()),
            "Foo class should still be in export_names()"
        );
    }

    #[test]
    fn kotlin_private_class_methods_not_extracted() {
        let mut parser = KotlinParser::new().unwrap();
        let source = "private class Hidden {\n    fun method() {}\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Hidden", "method").is_none(),
            "method of private class should NOT be indexed"
        );
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
