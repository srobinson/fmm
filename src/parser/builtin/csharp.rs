use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::{
    collect_matches, compile_query, has_modifier, make_parser, push_export,
};

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
        let parser = make_parser(&language, "C#")?;

        let class_query = compile_query(
            &language,
            "(class_declaration name: (identifier) @name)",
            "class",
        )?;
        let interface_query = compile_query(
            &language,
            "(interface_declaration name: (identifier) @name)",
            "interface",
        )?;
        let struct_query = compile_query(
            &language,
            "(struct_declaration name: (identifier) @name)",
            "struct",
        )?;
        let enum_query = compile_query(
            &language,
            "(enum_declaration name: (identifier) @name)",
            "enum",
        )?;
        let method_query = compile_query(
            &language,
            "(method_declaration name: (identifier) @name)",
            "method",
        )?;
        let using_query = compile_query(
            &language,
            "[(using_directive (identifier) @name) (using_directive (qualified_name) @name)]",
            "using",
        )?;
        let namespace_query = compile_query(
            &language,
            "(namespace_declaration name: (_) @name)",
            "namespace",
        )?;
        let attribute_query = compile_query(
            &language,
            "(attribute name: (identifier) @name)",
            "attribute",
        )?;

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

    fn collect_public_declarations(
        query: &Query,
        root_node: tree_sitter::Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Some(parent) = capture.node.parent() {
                    if has_modifier(&parent, source_bytes, "modifier", &["public"]) {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            push_export(
                                exports,
                                seen,
                                text.to_string(),
                                parent.start_position().row + 1,
                                parent.end_position().row + 1,
                            );
                        }
                    }
                }
            }
        }
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();

        let queries = [
            &self.class_query,
            &self.interface_query,
            &self.struct_query,
            &self.enum_query,
            &self.method_query,
        ];

        for query in queries {
            Self::collect_public_declarations(
                query,
                root_node,
                source_bytes,
                &mut seen,
                &mut exports,
            );
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let raw = collect_matches(&self.using_query, root_node, source.as_bytes());
        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        for full_path in &raw {
            let root_ns = full_path.split('.').next().unwrap_or(full_path).to_string();
            if seen.insert(root_ns.clone()) {
                imports.push(root_ns);
            }
        }
        imports.sort();
        imports
    }

    fn extract_namespaces(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.namespace_query, root_node, source.as_bytes())
    }

    fn extract_attributes(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.attribute_query, root_node, source.as_bytes())
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
                ..Default::default()
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

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "csharp",
        extensions: &["cs"],
        reexport_filenames: &[],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &[],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };

impl crate::parser::LanguageDescriptor for CSharpParser {
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
        let exports = &result.metadata.exports;
        assert!(result
            .metadata
            .export_names()
            .contains(&"UserService".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"CreateUser".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"Validate".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"InternalHelper".to_string()));

        // Exports should use declaration line ranges, NOT namespace range [2, 9]
        let user_service = exports.iter().find(|e| e.name == "UserService").unwrap();
        assert_eq!(user_service.start_line, 3); // "public class UserService {"
        assert_eq!(user_service.end_line, 6); // closing "}"
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
        assert!(result
            .metadata
            .export_names()
            .contains(&"IRepository".to_string()));
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
        // Qualified names now captured and extracted to root namespace
        assert!(result.metadata.imports.contains(&"Microsoft".to_string()));
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
