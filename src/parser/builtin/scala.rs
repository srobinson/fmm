use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

pub struct ScalaParser {
    parser: TSParser,
}

impl ScalaParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_scala::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Scala language: {}", e))?;

        Ok(Self { parser })
    }

    /// Check if a definition node has private or protected access modifier.
    fn is_private_or_protected(node: &tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "access_modifier" {
                        if let Ok(text) = mod_child.utf8_text(source_bytes) {
                            if text.starts_with("private") || text.starts_with("protected") {
                                return true;
                            }
                        }
                    }
                }
            }
            // modifiers come before the keyword — stop at class/trait/object/def/val/var
            if matches!(
                child.kind(),
                "class" | "trait" | "object" | "def" | "val" | "var" | "case"
            ) {
                break;
            }
        }
        false
    }

    /// Check if a class_definition has the `case` keyword.
    fn is_case_class(node: &tree_sitter::Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "case" {
                return true;
            }
            if child.kind() == "class" {
                break;
            }
        }
        false
    }

    /// Check if a definition has the `implicit` modifier.
    fn has_implicit(node: &tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    if text.contains("implicit") {
                        return true;
                    }
                }
            }
            if matches!(
                child.kind(),
                "class" | "trait" | "object" | "def" | "val" | "var" | "case"
            ) {
                break;
            }
        }
        false
    }

    /// Extract the name identifier from a definition node.
    fn extract_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        None
    }

    fn extract_exports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<ExportEntry>, Vec<String>, usize) {
        let source_bytes = source.as_bytes();
        let source_lines: Vec<&str> = source.lines().collect();
        let mut exports = Vec::new();
        let mut case_classes = Vec::new();
        let mut implicit_count = 0;

        // Collect start positions of ALL top-level definitions for end-line clamping
        let mut all_def_starts = Vec::new();
        let mut first_cursor = root_node.walk();
        for child in root_node.children(&mut first_cursor) {
            if matches!(
                child.kind(),
                "class_definition"
                    | "trait_definition"
                    | "object_definition"
                    | "function_definition"
                    | "val_definition"
                    | "var_definition"
            ) {
                all_def_starts.push(child.start_position().row + 1);
            }
        }
        all_def_starts.sort();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "class_definition" => {
                    if !Self::is_private_or_protected(&child, source_bytes) {
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            if Self::is_case_class(&child) {
                                case_classes.push(name.clone());
                            }
                            if Self::has_implicit(&child, source_bytes) {
                                implicit_count += 1;
                            }
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "trait_definition" => {
                    if !Self::is_private_or_protected(&child, source_bytes) {
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "object_definition" => {
                    if !Self::is_private_or_protected(&child, source_bytes) {
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "function_definition" => {
                    if !Self::is_private_or_protected(&child, source_bytes) {
                        if Self::has_implicit(&child, source_bytes) {
                            implicit_count += 1;
                        }
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "val_definition" | "var_definition" => {
                    if !Self::is_private_or_protected(&child, source_bytes) {
                        if Self::has_implicit(&child, source_bytes) {
                            implicit_count += 1;
                        }
                        if let Some(name) = Self::extract_name(&child, source_bytes) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Clamp end lines: don't bleed into next definition's range
        for export in &mut exports {
            if let Some(&next_start) = all_def_starts.iter().find(|&&s| s > export.start_line) {
                if export.end_line >= next_start {
                    export.end_line = next_start - 1;
                }
            }
            // Trim trailing blank lines
            while export.end_line > export.start_line {
                let line_idx = export.end_line - 1;
                if line_idx < source_lines.len() && source_lines[line_idx].trim().is_empty() {
                    export.end_line -= 1;
                } else {
                    break;
                }
            }
        }

        // Merge companion objects: same name → extend range to cover both
        let mut merged: Vec<ExportEntry> = Vec::new();
        let mut name_index: HashMap<String, usize> = HashMap::new();
        for export in exports {
            if let Some(&idx) = name_index.get(&export.name) {
                let existing = &mut merged[idx];
                existing.start_line = existing.start_line.min(export.start_line);
                existing.end_line = existing.end_line.max(export.end_line);
            } else {
                name_index.insert(export.name.clone(), merged.len());
                merged.push(export);
            }
        }

        case_classes.sort();
        merged.sort_by_key(|e| e.start_line);
        (merged, case_classes, implicit_count)
    }

    /// Extract imports from import_declaration nodes.
    /// The first identifier child is the root package.
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
                // First identifier child is the root package
                let mut inner_cursor = child.walk();
                for import_child in child.children(&mut inner_cursor) {
                    if import_child.kind() == "identifier" {
                        if let Ok(text) = import_child.utf8_text(source_bytes) {
                            import_set.insert(text.to_string());
                        }
                        break; // Only the first identifier (root package)
                    }
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        imports.sort();
        (imports, Vec::new()) // Scala doesn't have a clear "dependency" distinction
    }

    /// Extract annotations from all top-level definitions.
    fn extract_annotations(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut annotation_set = HashSet::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if matches!(
                child.kind(),
                "class_definition"
                    | "trait_definition"
                    | "object_definition"
                    | "function_definition"
                    | "val_definition"
                    | "var_definition"
            ) {
                Self::collect_annotations(&child, source_bytes, &mut annotation_set);
            }
        }

        let mut annotations: Vec<String> = annotation_set.into_iter().collect();
        annotations.sort();
        annotations
    }

    fn collect_annotations(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        annotations: &mut HashSet<String>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "annotation" {
                // Find the identifier inside the annotation
                let mut ann_cursor = child.walk();
                for ann_child in child.children(&mut ann_cursor) {
                    if ann_child.kind() == "identifier" || ann_child.kind() == "type_identifier" {
                        if let Ok(text) = ann_child.utf8_text(source_bytes) {
                            annotations.insert(text.to_string());
                        }
                        break;
                    }
                }
            }
            // Stop at the keyword
            if matches!(
                child.kind(),
                "class" | "trait" | "object" | "def" | "val" | "var" | "case"
            ) {
                break;
            }
        }
    }
}

impl Parser for ScalaParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Scala source"))?;

        let root_node = tree.root_node();
        let (exports, case_classes, implicit_count) = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let annotations = self.extract_annotations(source, root_node);
        let loc = source.lines().count();

        let mut custom_fields = HashMap::new();
        if !case_classes.is_empty() {
            custom_fields.insert(
                "case_classes".to_string(),
                serde_json::Value::Array(
                    case_classes
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        if implicit_count > 0 {
            custom_fields.insert(
                "implicits".to_string(),
                serde_json::Value::Number(implicit_count.into()),
            );
        }
        if !annotations.is_empty() {
            custom_fields.insert(
                "annotations".to_string(),
                serde_json::Value::Array(
                    annotations
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
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
        "scala"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scala_classes() {
        let mut parser = ScalaParser::new().unwrap();
        let source = "class Foo\ncase class Bar(x: Int)\nprivate class Baz\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Foo".to_string()));
        assert!(names.contains(&"Bar".to_string()));
        assert!(!names.contains(&"Baz".to_string()));
    }

    #[test]
    fn parse_scala_traits_and_objects() {
        let mut parser = ScalaParser::new().unwrap();
        let source = "trait Service\nobject Config\nprivate object Internal\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Service".to_string()));
        assert!(names.contains(&"Config".to_string()));
        assert!(!names.contains(&"Internal".to_string()));
    }

    #[test]
    fn parse_scala_functions() {
        let mut parser = ScalaParser::new().unwrap();
        let source = "def process(x: Int): Int = x\nprivate def helper(): Unit = ()\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"process".to_string()));
        assert!(!names.contains(&"helper".to_string()));
    }

    #[test]
    fn parse_scala_imports() {
        let mut parser = ScalaParser::new().unwrap();
        let source = "import scala.collection.mutable\nimport akka.actor.Actor\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"scala".to_string()));
        assert!(result.metadata.imports.contains(&"akka".to_string()));
    }

    #[test]
    fn parse_scala_case_classes_custom_field() {
        let mut parser = ScalaParser::new().unwrap();
        let source = "case class Foo(x: Int)\nclass Bar\ncase class Baz(y: String)\n";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let cc = fields.get("case_classes").unwrap().as_array().unwrap();
        let names: Vec<&str> = cc.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"Baz"));
        assert!(!names.contains(&"Bar"));
    }

    #[test]
    fn parse_scala_empty() {
        let mut parser = ScalaParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
