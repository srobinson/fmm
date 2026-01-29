use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct GoParser {
    parser: TSParser,
    func_query: Query,
    type_query: Query,
    const_query: Query,
    var_query: Query,
    import_query: Query,
}

impl GoParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_go::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Go language: {}", e))?;

        let func_query = Query::new(
            &language,
            "(source_file (function_declaration name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile func query: {}", e))?;

        let type_query = Query::new(
            &language,
            "(source_file (type_declaration (type_spec name: (type_identifier) @name)))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile type query: {}", e))?;

        let const_query = Query::new(
            &language,
            "(source_file (const_declaration (const_spec name: (identifier) @name)))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile const query: {}", e))?;

        let var_query = Query::new(
            &language,
            "(source_file (var_declaration (var_spec name: (identifier) @name)))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile var query: {}", e))?;

        let import_query = Query::new(
            &language,
            "(import_spec path: (interpreted_string_literal) @path)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile import query: {}", e))?;

        Ok(Self {
            parser,
            func_query,
            type_query,
            const_query,
            var_query,
            import_query,
        })
    }

    fn is_exported(name: &str) -> bool {
        name.starts_with(|c: char| c.is_uppercase())
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        let queries = [
            &self.func_query,
            &self.type_query,
            &self.const_query,
            &self.var_query,
        ];

        for query in queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        if Self::is_exported(text) && !exports.contains(&text.to_string()) {
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

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let mut imports = Vec::new();
        let mut dependencies = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let path = text.trim_matches('"');
                    if path.is_empty() {
                        continue;
                    }
                    // Local/relative imports contain a dot (e.g., "./pkg" or "github.com/user/repo/pkg")
                    // Standard library imports have no dot in the first segment
                    let root_pkg = path.split('/').next().unwrap_or(path);
                    if root_pkg.contains('.') {
                        // External dependency (e.g., "github.com/...")
                        if !dependencies.contains(&path.to_string()) {
                            dependencies.push(path.to_string());
                        }
                    } else {
                        // Standard library or simple package
                        if !imports.contains(&path.to_string()) {
                            imports.push(path.to_string());
                        }
                    }
                }
            }
        }

        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }
}

impl Parser for GoParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Go source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let loc = source.lines().count();

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
            },
            custom_fields: None,
        })
    }

    fn language_id(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_exported_functions() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

func ExportedFunc() {}
func unexportedFunc() {}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .exports
            .contains(&"ExportedFunc".to_string()));
        assert!(!result
            .metadata
            .exports
            .contains(&"unexportedFunc".to_string()));
    }

    #[test]
    fn parse_go_exported_types() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

type Config struct {
    Name string
}

type handler struct {
    count int
}

type Service interface {
    Start() error
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"Config".to_string()));
        assert!(result.metadata.exports.contains(&"Service".to_string()));
        assert!(!result.metadata.exports.contains(&"handler".to_string()));
    }

    #[test]
    fn parse_go_imports() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

import (
    "fmt"
    "os"
    "net/http"
    "github.com/gin-gonic/gin"
)
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"fmt".to_string()));
        assert!(result.metadata.imports.contains(&"os".to_string()));
        assert!(result.metadata.imports.contains(&"net/http".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"github.com/gin-gonic/gin".to_string()));
    }

    #[test]
    fn parse_go_constants_and_vars() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

const MaxRetries = 3
const internalLimit = 10

var GlobalState = "init"
var localVar = "hidden"
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"MaxRetries".to_string()));
        assert!(result.metadata.exports.contains(&"GlobalState".to_string()));
        assert!(!result
            .metadata
            .exports
            .contains(&"internalLimit".to_string()));
        assert!(!result.metadata.exports.contains(&"localVar".to_string()));
    }

    #[test]
    fn parse_go_empty() {
        let mut parser = GoParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
