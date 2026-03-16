use super::query_helpers::{collect_matches, compile_query, make_parser};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct CppParser {
    parser: TSParser,
    func_query: Query,
    class_query: Query,
    struct_query: Query,
    enum_query: Query,
    namespace_query: Query,
    system_include_query: Query,
    local_include_query: Query,
    template_query: Query,
}

impl CppParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_cpp::LANGUAGE.into();
        let parser = make_parser(&language, "C++")?;

        let func_query = compile_query(
            &language,
            "(function_definition declarator: (function_declarator declarator: (identifier) @name))",
            "func",
        )?;
        let class_query = compile_query(
            &language,
            "(class_specifier name: (type_identifier) @name)",
            "class",
        )?;
        let struct_query = compile_query(
            &language,
            "(struct_specifier name: (type_identifier) @name)",
            "struct",
        )?;
        let enum_query = compile_query(
            &language,
            "(enum_specifier name: (type_identifier) @name)",
            "enum",
        )?;
        let namespace_query = compile_query(
            &language,
            "(namespace_definition name: (namespace_identifier) @name)",
            "namespace",
        )?;
        let system_include_query = compile_query(
            &language,
            "(preproc_include path: (system_lib_string) @path)",
            "system include",
        )?;
        let local_include_query = compile_query(
            &language,
            "(preproc_include path: (string_literal) @path)",
            "local include",
        )?;
        let template_query = compile_query(
            &language,
            "(template_declaration (class_specifier name: (type_identifier) @name))",
            "template",
        )?;

        Ok(Self {
            parser,
            func_query,
            class_query,
            struct_query,
            enum_query,
            namespace_query,
            system_include_query,
            local_include_query,
            template_query,
        })
    }

    /// Walk up from a captured node to find the declaration-level ancestor.
    /// Stops at the node whose parent is the root (translation_unit) or a
    /// declaration_list (inside a namespace). This prevents walking up into
    /// the enclosing namespace_definition.
    fn declaration_ancestor(node: tree_sitter::Node) -> tree_sitter::Node {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if parent.parent().is_none() || parent.kind() == "declaration_list" {
                return current;
            }
            current = parent;
        }
        current
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let queries = [
            &self.func_query,
            &self.class_query,
            &self.struct_query,
            &self.enum_query,
            &self.template_query,
        ];

        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        for query in queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        if seen.insert(name.clone()) {
                            let decl = Self::declaration_ancestor(capture.node);
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

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.system_include_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    seen.insert(
                        text.trim_start_matches('<')
                            .trim_end_matches('>')
                            .to_string(),
                    );
                }
            }
        }

        let mut imports: Vec<String> = seen.into_iter().collect();
        imports.sort();
        imports
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.local_include_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    seen.insert(text.trim_matches('"').to_string());
                }
            }
        }

        let mut deps: Vec<String> = seen.into_iter().collect();
        deps.sort();
        deps
    }

    fn extract_namespaces(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.namespace_query, root_node, source.as_bytes())
    }
}

impl Parser for CppParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse C++ source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        let namespaces = self.extract_namespaces(source, root_node);
        let custom_fields = if namespaces.is_empty() {
            None
        } else {
            let mut fields = HashMap::new();
            fields.insert(
                "namespaces".to_string(),
                serde_json::Value::Array(
                    namespaces
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
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "hpp", "cc", "hh", "cxx", "hxx"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "cpp",
        extensions: &["cpp", "hpp", "cc", "hh", "cxx", "hxx"],
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
    fn parse_cpp_functions() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
int main() {
    return 0;
}

void processData() {
    // do stuff
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.export_names().contains(&"main".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"processData".to_string()));
    }

    #[test]
    fn parse_cpp_classes() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
class Engine {
public:
    void start();
private:
    int rpm;
};

struct Point {
    double x, y;
};
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Engine".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"Point".to_string()));
    }

    #[test]
    fn parse_cpp_includes() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
#include <iostream>
#include <vector>
#include "config.h"
#include "utils/helpers.h"
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"iostream".to_string()));
        assert!(result.metadata.imports.contains(&"vector".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"config.h".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"utils/helpers.h".to_string()));
    }

    #[test]
    fn parse_cpp_namespaces() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
namespace engine {
    class Core {};
}

namespace utils {
    void helper() {}
}
"#;
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
        let names: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"engine"));
        assert!(names.contains(&"utils"));

        // Exports should use declaration line ranges, NOT namespace range
        let exports = &result.metadata.exports;
        let core = exports.iter().find(|e| e.name == "Core").unwrap();
        assert_eq!(core.start_line, 3); // "class Core {};"
        let helper = exports.iter().find(|e| e.name == "helper").unwrap();
        assert_eq!(helper.start_line, 7); // "void helper() {}"
    }

    #[test]
    fn parse_cpp_templates() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
template <typename T>
class Container {
    T value;
};
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Container".to_string()));
    }

    #[test]
    fn parse_cpp_empty() {
        let mut parser = CppParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
