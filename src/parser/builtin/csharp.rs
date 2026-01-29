use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct CSharpParser {
    parser: TSParser,
    class_query: Query,
    interface_query: Query,
    struct_query: Query,
    enum_query: Query,
    method_query: Query,
    using_query: Query,
    namespace_query: Query,
    attribute_query: Query,
}

impl CSharpParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_c_sharp::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set C# language: {}", e))?;

        let class_query = Query::new(&language, "(class_declaration name: (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile class query: {}", e))?;

        let interface_query = Query::new(
            &language,
            "(interface_declaration name: (identifier) @name)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile interface query: {}", e))?;

        let struct_query = Query::new(&language, "(struct_declaration name: (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile struct query: {}", e))?;

        let enum_query = Query::new(&language, "(enum_declaration name: (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile enum query: {}", e))?;

        let method_query = Query::new(&language, "(method_declaration name: (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile method query: {}", e))?;

        let using_query = Query::new(&language, "(using_directive (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile using query: {}", e))?;

        let namespace_query = Query::new(&language, "(namespace_declaration name: (_) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile namespace query: {}", e))?;

        let attribute_query = Query::new(&language, "(attribute name: (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile attribute query: {}", e))?;

        Ok(Self {
            parser,
            class_query,
            interface_query,
            struct_query,
            enum_query,
            method_query,
            using_query,
            namespace_query,
            attribute_query,
        })
    }

    fn has_public_modifier(node: tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    if text == "public" {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        // Public classes
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.class_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if Self::has_public_modifier(parent, source_bytes) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            if !exports.contains(&text.to_string()) {
                                exports.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Public interfaces
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.interface_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if Self::has_public_modifier(parent, source_bytes) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            if !exports.contains(&text.to_string()) {
                                exports.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Public structs
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.struct_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if Self::has_public_modifier(parent, source_bytes) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            if !exports.contains(&text.to_string()) {
                                exports.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Public enums
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.enum_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if Self::has_public_modifier(parent, source_bytes) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            if !exports.contains(&text.to_string()) {
                                exports.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Public methods
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.method_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if Self::has_public_modifier(parent, source_bytes) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
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

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.using_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let ns = text.to_string();
                    if !imports.contains(&ns) {
                        imports.push(ns);
                    }
                }
            }
        }

        imports.sort();
        imports
    }

    fn extract_namespaces(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut namespaces = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.namespace_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let name = text.to_string();
                    if !namespaces.contains(&name) {
                        namespaces.push(name);
                    }
                }
            }
        }

        namespaces.sort();
        namespaces
    }

    fn extract_attributes(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut attributes = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.attribute_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let name = text.to_string();
                    if !attributes.contains(&name) {
                        attributes.push(name);
                    }
                }
            }
        }

        attributes.sort();
        attributes
    }
}

impl Parser for CSharpParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse C# source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let loc = source.lines().count();

        let namespaces = self.extract_namespaces(source, root_node);
        let attributes = self.extract_attributes(source, root_node);

        let mut custom_fields_map = HashMap::new();
        if !namespaces.is_empty() {
            custom_fields_map.insert(
                "namespaces".to_string(),
                serde_json::Value::Array(
                    namespaces
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        if !attributes.is_empty() {
            custom_fields_map.insert(
                "attributes".to_string(),
                serde_json::Value::Array(
                    attributes
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        let custom_fields = if custom_fields_map.is_empty() {
            None
        } else {
            Some(custom_fields_map)
        };

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies: Vec::new(),
                loc,
            },
            custom_fields,
        })
    }

    fn language_id(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csharp_public_class() {
        let mut parser = CSharpParser::new().unwrap();
        let source = r#"
namespace MyApp {
    public class UserService {
        public void CreateUser() {}
        private void Validate() {}
    }

    internal class InternalHelper {}
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"UserService".to_string()));
        assert!(result.metadata.exports.contains(&"CreateUser".to_string()));
        assert!(!result.metadata.exports.contains(&"Validate".to_string()));
        assert!(!result
            .metadata
            .exports
            .contains(&"InternalHelper".to_string()));
    }

    #[test]
    fn parse_csharp_interfaces() {
        let mut parser = CSharpParser::new().unwrap();
        let source = r#"
public interface IRepository<T> {
    T FindById(int id);
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"IRepository".to_string()));
    }

    #[test]
    fn parse_csharp_using_statements() {
        let mut parser = CSharpParser::new().unwrap();
        let source = r#"
using System;
using System.Collections.Generic;
using Microsoft.Extensions.DependencyInjection;

public class App {}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"System".to_string()));
        // Note: using with dotted names - we capture what tree-sitter gives us
    }

    #[test]
    fn parse_csharp_namespaces() {
        let mut parser = CSharpParser::new().unwrap();
        let source = r#"
namespace MyApp.Services {
    public class Service {}
}
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
        assert!(!namespaces.is_empty());
    }

    #[test]
    fn parse_csharp_attributes() {
        let mut parser = CSharpParser::new().unwrap();
        let source = r#"
[Serializable]
[Obsolete]
public class Config {
    [Required]
    public string Name { get; set; }
}
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let attributes = fields.get("attributes").unwrap().as_array().unwrap();
        let names: Vec<&str> = attributes.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Serializable"));
        assert!(names.contains(&"Obsolete"));
        assert!(names.contains(&"Required"));
    }

    #[test]
    fn parse_csharp_empty() {
        let mut parser = CSharpParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
