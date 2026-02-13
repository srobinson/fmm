use super::query_helpers::{collect_matches_with_lines, collect_named_matches, top_level_ancestor};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct RubyParser {
    parser: TSParser,
    class_query: Query,
    module_query: Query,
    method_query: Query,
    require_query: Query,
    require_relative_query: Query,
}

impl RubyParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_ruby::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Ruby language: {}", e))?;

        let class_query = Query::new(&language, "(program (class name: (constant) @name))")
            .map_err(|e| anyhow::anyhow!("Failed to compile class query: {}", e))?;

        let module_query = Query::new(&language, "(program (module name: (constant) @name))")
            .map_err(|e| anyhow::anyhow!("Failed to compile module query: {}", e))?;

        let method_query = Query::new(&language, "(program (method name: (identifier) @name))")
            .map_err(|e| anyhow::anyhow!("Failed to compile method query: {}", e))?;

        let require_query = Query::new(
            &language,
            "(call method: (identifier) @method arguments: (argument_list (string (string_content) @path)) (#eq? @method \"require\"))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile require query: {}", e))?;

        let require_relative_query = Query::new(
            &language,
            "(call method: (identifier) @method arguments: (argument_list (string (string_content) @path)) (#eq? @method \"require_relative\"))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile require_relative query: {}", e))?;

        Ok(Self {
            parser,
            class_query,
            module_query,
            method_query,
            require_query,
            require_relative_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();

        let mut seen: HashSet<String> = HashSet::new();
        let mut exports = Vec::new();

        for entry in collect_matches_with_lines(&self.class_query, root_node, source_bytes) {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }

        for entry in collect_matches_with_lines(&self.module_query, root_node, source_bytes) {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.method_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if !text.starts_with('_') {
                        let name = text.to_string();
                        if seen.insert(name.clone()) {
                            let decl = top_level_ancestor(capture.node);
                            exports.push(ExportEntry::new(
                                name,
                                decl.start_position().row + 1,
                                decl.end_position().row + 1,
                            ));
                        }
                    }
                }
            }
        }

        exports.sort_by(|a, b| a.name.cmp(&b.name));
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_named_matches(&self.require_query, "path", root_node, source.as_bytes())
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_named_matches(
            &self.require_relative_query,
            "path",
            root_node,
            source.as_bytes(),
        )
    }

    fn extract_mixins(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let source_bytes = source.as_bytes();

        self.collect_mixins(root_node, source_bytes, &mut seen);

        let mut mixins: Vec<String> = seen.into_iter().collect();
        mixins.sort();
        mixins
    }

    fn collect_mixins(
        &self,
        node: tree_sitter::Node,
        source_bytes: &[u8],
        mixins: &mut HashSet<String>,
    ) {
        if node.kind() == "call" {
            let mut cursor = node.walk();
            let mut method_name = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Ok(text) = child.utf8_text(source_bytes) {
                        if text == "include" || text == "extend" || text == "prepend" {
                            method_name = Some(text.to_string());
                        }
                    }
                }
                if child.kind() == "argument_list" && method_name.is_some() {
                    let mut arg_cursor = child.walk();
                    for arg in child.children(&mut arg_cursor) {
                        if arg.kind() == "constant" || arg.kind() == "scope_resolution" {
                            if let Ok(text) = arg.utf8_text(source_bytes) {
                                mixins.insert(text.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_mixins(child, source_bytes, mixins);
        }
    }
}

impl Parser for RubyParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Ruby source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        let mixins = self.extract_mixins(source, root_node);
        let custom_fields = if mixins.is_empty() {
            None
        } else {
            let mut fields = HashMap::new();
            fields.insert(
                "mixins".to_string(),
                serde_json::Value::Array(
                    mixins.into_iter().map(serde_json::Value::String).collect(),
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
        "ruby"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ruby_classes() {
        let mut parser = RubyParser::new().unwrap();
        let source = r#"
class UserService
  def initialize(name)
    @name = name
  end

  def process
    # do stuff
  end
end
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"UserService".to_string()));
    }

    #[test]
    fn parse_ruby_modules() {
        let mut parser = RubyParser::new().unwrap();
        let source = r#"
module Serializable
  def serialize
    to_json
  end
end

module Cacheable
  def cache_key
    "key"
  end
end
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Serializable".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"Cacheable".to_string()));
    }

    #[test]
    fn parse_ruby_top_level_methods() {
        let mut parser = RubyParser::new().unwrap();
        let source = r#"
def helper_method
  "help"
end

def _private_method
  "private"
end
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"helper_method".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"_private_method".to_string()));
    }

    #[test]
    fn parse_ruby_requires() {
        let mut parser = RubyParser::new().unwrap();
        let source = r#"
require 'json'
require 'net/http'
require_relative 'config'
require_relative 'lib/helpers'
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"json".to_string()));
        assert!(result.metadata.imports.contains(&"net/http".to_string()));
        assert!(result.metadata.dependencies.contains(&"config".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"lib/helpers".to_string()));
    }

    #[test]
    fn parse_ruby_mixins() {
        let mut parser = RubyParser::new().unwrap();
        let source = r#"
class User
  include Comparable
  extend ClassMethods
  prepend Validatable
end
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let mixins = fields.get("mixins").unwrap().as_array().unwrap();
        let names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Comparable"));
        assert!(names.contains(&"ClassMethods"));
        assert!(names.contains(&"Validatable"));
    }

    #[test]
    fn parse_ruby_empty() {
        let mut parser = RubyParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
