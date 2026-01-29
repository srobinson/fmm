use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashMap;
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
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set C++ language: {}", e))?;

        let func_query = Query::new(
            &language,
            "(function_definition declarator: (function_declarator declarator: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile func query: {}", e))?;

        let class_query = Query::new(&language, "(class_specifier name: (type_identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile class query: {}", e))?;

        let struct_query = Query::new(
            &language,
            "(struct_specifier name: (type_identifier) @name)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile struct query: {}", e))?;

        let enum_query = Query::new(&language, "(enum_specifier name: (type_identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile enum query: {}", e))?;

        let namespace_query = Query::new(
            &language,
            "(namespace_definition name: (namespace_identifier) @name)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile namespace query: {}", e))?;

        let system_include_query = Query::new(
            &language,
            "(preproc_include path: (system_lib_string) @path)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile system include query: {}", e))?;

        let local_include_query =
            Query::new(&language, "(preproc_include path: (string_literal) @path)")
                .map_err(|e| anyhow::anyhow!("Failed to compile local include query: {}", e))?;

        let template_query = Query::new(
            &language,
            "(template_declaration (class_specifier name: (type_identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile template query: {}", e))?;

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

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        let queries = [
            &self.func_query,
            &self.class_query,
            &self.struct_query,
            &self.enum_query,
            &self.template_query,
        ];

        for query in queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        if !exports.contains(&text.to_string()) {
                            exports.push(text.to_string());
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

        // System includes: #include <header>
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.system_include_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let header = text
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_string();
                    if !imports.contains(&header) {
                        imports.push(header);
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

        // Local includes: #include "header.h"
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.local_include_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let header = text.trim_matches('"').to_string();
                    if !deps.contains(&header) {
                        deps.push(header);
                    }
                }
            }
        }

        deps.sort();
        deps
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
        assert!(result.metadata.exports.contains(&"main".to_string()));
        assert!(result.metadata.exports.contains(&"processData".to_string()));
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
        assert!(result.metadata.exports.contains(&"Engine".to_string()));
        assert!(result.metadata.exports.contains(&"Point".to_string()));
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
        assert!(result.metadata.exports.contains(&"Container".to_string()));
    }

    #[test]
    fn parse_cpp_empty() {
        let mut parser = CppParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
