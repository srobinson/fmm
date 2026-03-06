use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::{
    collect_matches, collect_matches_with_lines, compile_query, make_parser,
};

pub struct JavaParser {
    parser: TSParser,
    class_query: Query,
    interface_query: Query,
    enum_query: Query,
    method_query: Query,
    import_query: Query,
    annotation_query: Query,
}

impl JavaParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_java::LANGUAGE.into();
        let parser = make_parser(&language, "Java")?;

        let class_query = compile_query(
            &language,
            "(program (class_declaration name: (identifier) @name))",
            "class",
        )?;
        let interface_query = compile_query(
            &language,
            "(program (interface_declaration name: (identifier) @name))",
            "interface",
        )?;
        let enum_query = compile_query(
            &language,
            "(program (enum_declaration name: (identifier) @name))",
            "enum",
        )?;
        // ALP-771: capture class_name alongside method_name so methods can be attributed to parent
        let method_query = compile_query(
            &language,
            "(class_declaration name: (identifier) @class_name body: (class_body (method_declaration name: (identifier) @method_name)))",
            "method",
        )?;
        let import_query = compile_query(
            &language,
            "(import_declaration (scoped_identifier) @path)",
            "import",
        )?;
        let annotation_query = compile_query(
            &language,
            "(marker_annotation name: (identifier) @name)",
            "annotation",
        )?;

        Ok(Self {
            parser,
            class_query,
            interface_query,
            enum_query,
            method_query,
            import_query,
            annotation_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();

        for entry in collect_matches_with_lines(&self.class_query, root_node, source_bytes) {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }
        for entry in collect_matches_with_lines(&self.interface_query, root_node, source_bytes) {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }
        for entry in collect_matches_with_lines(&self.enum_query, root_node, source_bytes) {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }

        // ALP-771: attribute methods to their parent class using parent_class field
        let class_name_idx = self
            .method_query
            .capture_index_for_name("class_name")
            .unwrap_or(0);
        let method_name_idx = self
            .method_query
            .capture_index_for_name("method_name")
            .unwrap_or(1);

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.method_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            let class_name = m
                .captures
                .iter()
                .find(|c| c.index == class_name_idx)
                .and_then(|c| c.node.utf8_text(source_bytes).ok())
                .map(|s| s.to_string());
            let method_cap = m.captures.iter().find(|c| c.index == method_name_idx);

            if let (Some(class_name), Some(method_cap)) = (class_name, method_cap) {
                // Skip methods from inner/private classes not in the exported set.
                // At this point seen contains only top-level class/interface/enum names.
                if !seen.contains(&class_name) {
                    continue;
                }
                let method_node = method_cap.node;
                if let Some(method_decl) = method_node.parent() {
                    if self.has_public_modifier(method_decl, source_bytes) {
                        if let Ok(text) = method_node.utf8_text(source_bytes) {
                            let method_name = text.to_string();
                            // Use "ClassName.method" as the dedup key to scope correctly
                            let key = format!("{}.{}", class_name, method_name);
                            if seen.insert(key) {
                                exports.push(ExportEntry::method(
                                    method_name,
                                    method_decl.start_position().row + 1,
                                    method_decl.end_position().row + 1,
                                    class_name,
                                ));
                            }
                        }
                    }
                }
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    fn has_public_modifier(&self, node: tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    if modifier.kind() == "public" {
                        return true;
                    }
                    if let Ok(text) = modifier.utf8_text(source_bytes) {
                        if text == "public" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let raw = collect_matches(&self.import_query, root_node, source_bytes);

        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        for full_path in &raw {
            let segments: Vec<&str> = full_path.split('.').collect();
            let pkg = if segments.len() >= 2 {
                format!("{}.{}", segments[0], segments[1])
            } else {
                segments[0].to_string()
            };
            if seen.insert(pkg.clone()) {
                imports.push(pkg);
            }
        }

        imports.sort();
        imports
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.import_query, root_node, source.as_bytes())
    }

    fn extract_annotations(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.annotation_query, root_node, source.as_bytes())
    }
}

impl Parser for JavaParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Java source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        let annotations = self.extract_annotations(source, root_node);
        let custom_fields = if annotations.is_empty() {
            None
        } else {
            let mut fields = HashMap::new();
            fields.insert(
                "annotations".to_string(),
                serde_json::Value::Array(
                    annotations
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
            Some(fields)
        };

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
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_java_classes() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public class UserService {
    public void createUser() {}
    private void validate() {}
}
"#;
        let result = parser.parse(source).unwrap();
        // Class itself is a top-level export
        assert!(result
            .metadata
            .export_names()
            .contains(&"UserService".to_string()));
        // ALP-771: public methods are now method entries with parent_class, NOT in export_names()
        assert!(
            !result
                .metadata
                .export_names()
                .contains(&"createUser".to_string()),
            "createUser should NOT be in flat export_names() — it's a method entry"
        );
        assert!(
            get_method(&result.metadata.exports, "UserService", "createUser").is_some(),
            "UserService.createUser should be in method entries"
        );
        assert!(
            get_method(&result.metadata.exports, "UserService", "validate").is_none(),
            "private validate() should NOT be indexed"
        );
    }

    #[test]
    fn java_method_has_correct_parent_class() {
        let mut parser = JavaParser::new().unwrap();
        let source = "public class Foo {\n    public void bar() {}\n}\n";
        let result = parser.parse(source).unwrap();
        let entry =
            get_method(&result.metadata.exports, "Foo", "bar").expect("Foo.bar should be indexed");
        assert_eq!(entry.parent_class.as_deref(), Some("Foo"));
    }

    #[test]
    fn java_method_not_in_flat_export_names() {
        // Regression: fmm_lookup_export("bar") should return not-found
        let mut parser = JavaParser::new().unwrap();
        let source = "public class Foo {\n    public void bar() {}\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            !result.metadata.export_names().contains(&"bar".to_string()),
            "bar should NOT appear in flat export index"
        );
    }

    #[test]
    fn java_inner_class_methods_not_indexed() {
        // The method_query matches all class_declaration nodes. Without the exported-class
        // guard, methods from private/package-private inner classes would leak into the index.
        let mut parser = JavaParser::new().unwrap();
        let source = "public class Outer {\n    public void outerMethod() {}\n    private class Inner {\n        public void innerMethod() {}\n    }\n}\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Outer", "outerMethod").is_some(),
            "Outer.outerMethod should be indexed"
        );
        assert!(
            get_method(&result.metadata.exports, "Inner", "innerMethod").is_none(),
            "Inner.innerMethod should NOT be indexed — Inner is not a top-level exported class"
        );
    }

    #[test]
    fn parse_java_interfaces() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public interface Repository<T> {
    T findById(long id);
    List<T> findAll();
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Repository".to_string()));
    }

    #[test]
    fn parse_java_imports() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
import java.util.List;
import java.util.Map;
import org.springframework.stereotype.Service;

public class App {}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"java.util".to_string()));
        assert!(result
            .metadata
            .imports
            .contains(&"org.springframework".to_string()));
    }

    #[test]
    fn parse_java_annotations() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
import org.springframework.stereotype.Service;

@Service
@Deprecated
public class UserService {
    @Override
    public String toString() { return ""; }
}
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let annotations = fields.get("annotations").unwrap().as_array().unwrap();
        let names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Service"));
        assert!(names.contains(&"Deprecated"));
        assert!(names.contains(&"Override"));
    }

    #[test]
    fn parse_java_enums() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public enum Status {
    ACTIVE, INACTIVE, PENDING
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Status".to_string()));
    }

    #[test]
    fn parse_java_empty() {
        let mut parser = JavaParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
