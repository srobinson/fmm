use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

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
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Java language: {}", e))?;

        let class_query = Query::new(
            &language,
            "(program (class_declaration name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile class query: {}", e))?;

        let interface_query = Query::new(
            &language,
            "(program (interface_declaration name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile interface query: {}", e))?;

        let enum_query = Query::new(
            &language,
            "(program (enum_declaration name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile enum query: {}", e))?;

        let method_query = Query::new(
            &language,
            "(class_declaration body: (class_body (method_declaration name: (identifier) @name)))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile method query: {}", e))?;

        let import_query = Query::new(&language, "(import_declaration (scoped_identifier) @path)")
            .map_err(|e| anyhow::anyhow!("Failed to compile import query: {}", e))?;

        let annotation_query =
            Query::new(&language, "(marker_annotation name: (identifier) @name)")
                .map_err(|e| anyhow::anyhow!("Failed to compile annotation query: {}", e))?;

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

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        // Top-level classes
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.class_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if !exports.contains(&text.to_string()) {
                        exports.push(text.to_string());
                    }
                }
            }
        }

        // Top-level interfaces
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.interface_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if !exports.contains(&text.to_string()) {
                        exports.push(text.to_string());
                    }
                }
            }
        }

        // Top-level enums
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.enum_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if !exports.contains(&text.to_string()) {
                        exports.push(text.to_string());
                    }
                }
            }
        }

        // Public methods in top-level classes
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.method_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                let method_node = capture.node;
                // Check if the method declaration has public modifier
                if let Some(method_decl) = method_node.parent() {
                    if self.has_public_modifier(method_decl, source_bytes) {
                        if let Ok(text) = method_node.utf8_text(source_bytes) {
                            if !exports.contains(&text.to_string()) {
                                exports.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        exports.sort();
        exports.dedup();
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
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let pkg = text.to_string();
                    // Extract root package (e.g., "java" from "java.util.List")
                    let root_pkg = pkg.split('.').next().unwrap_or(&pkg).to_string();
                    if !imports.contains(&root_pkg) {
                        imports.push(root_pkg);
                    }
                }
            }
        }

        imports.sort();
        imports
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut deps = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let full_path = text.to_string();
                    if !deps.contains(&full_path) {
                        deps.push(full_path);
                    }
                }
            }
        }

        deps.sort();
        deps
    }

    fn extract_annotations(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut annotations = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.annotation_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let name = text.to_string();
                    if !annotations.contains(&name) {
                        annotations.push(name);
                    }
                }
            }
        }

        annotations.sort();
        annotations
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
        assert!(result.metadata.exports.contains(&"UserService".to_string()));
        assert!(result.metadata.exports.contains(&"createUser".to_string()));
        assert!(!result.metadata.exports.contains(&"validate".to_string()));
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
        assert!(result.metadata.exports.contains(&"Repository".to_string()));
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
        assert!(result.metadata.imports.contains(&"java".to_string()));
        assert!(result.metadata.imports.contains(&"org".to_string()));
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
        assert!(result.metadata.exports.contains(&"Status".to_string()));
    }

    #[test]
    fn parse_java_empty() {
        let mut parser = JavaParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
