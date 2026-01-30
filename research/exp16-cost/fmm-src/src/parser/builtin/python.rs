use super::query_helpers::collect_matches;
use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct PythonParser {
    parser: TSParser,
    func_query: Query,
    class_query: Query,
    assign_query: Query,
    dunder_all_query: Query,
    import_queries: Vec<Query>,
    from_import_query: Query,
    relative_import_query: Query,
    decorator_query: Query,
    dotted_decorator_query: Query,
}

impl PythonParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Python language: {}", e))?;

        let func_query = Query::new(
            &language,
            "(module (function_definition name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile func query: {}", e))?;
        let class_query = Query::new(
            &language,
            "(module (class_definition name: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile class query: {}", e))?;
        let assign_query = Query::new(
            &language,
            "(module (expression_statement (assignment left: (identifier) @name)))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile assign query: {}", e))?;
        let dunder_all_query = Query::new(&language, "(module (expression_statement (assignment left: (identifier) @name right: (list) @values)))")
            .map_err(|e| anyhow::anyhow!("Failed to compile dunder_all query: {}", e))?;

        let import_queries = vec![
            Query::new(&language, "(import_statement name: (dotted_name) @name)")
                .map_err(|e| anyhow::anyhow!("Failed to compile import query: {}", e))?,
            Query::new(
                &language,
                "(import_statement name: (aliased_import name: (dotted_name) @name))",
            )
            .map_err(|e| anyhow::anyhow!("Failed to compile aliased import query: {}", e))?,
        ];

        let from_import_query = Query::new(
            &language,
            "(import_from_statement module_name: (dotted_name) @name)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile from_import query: {}", e))?;
        let relative_import_query = Query::new(
            &language,
            "(import_from_statement module_name: (relative_import) @name)",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile relative_import query: {}", e))?;
        let decorator_query = Query::new(&language, "(decorator (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile decorator query: {}", e))?;
        let dotted_decorator_query = Query::new(&language, "(decorator (attribute) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile dotted_decorator query: {}", e))?;

        Ok(Self {
            parser,
            func_query,
            class_query,
            assign_query,
            dunder_all_query,
            import_queries,
            from_import_query,
            relative_import_query,
            decorator_query,
            dotted_decorator_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        if let Some(all_exports) = self.extract_dunder_all(source, root_node) {
            return all_exports;
        }

        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        let mut collect_filtered = |query: &Query, filter: fn(&str) -> bool| {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        if !text.starts_with('_') && filter(text) && seen.insert(text.to_string()) {
                            exports.push(text.to_string());
                        }
                    }
                }
            }
        };

        // Top-level function definitions
        collect_filtered(&self.func_query, |_| true);

        // Top-level class definitions
        collect_filtered(&self.class_query, |_| true);

        // Top-level assignments (module-level constants)
        collect_filtered(&self.assign_query, |name| {
            name.chars().all(|c| c.is_uppercase() || c == '_')
                || name.chars().next().is_some_and(|c| c.is_uppercase())
        });

        exports.sort();
        exports
    }

    /// Extract names from `__all__ = [...]` if present.
    fn extract_dunder_all(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Option<Vec<String>> {
        let source_bytes = source.as_bytes();
        let capture_names = self.dunder_all_query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.dunder_all_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            let name_capture = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "name"
            })?;
            let values_capture = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "values"
            })?;

            if name_capture.node.utf8_text(source_bytes).ok()? != "__all__" {
                continue;
            }

            let mut seen = HashSet::new();
            let mut exports = Vec::new();
            let list_node = values_capture.node;
            let mut child_cursor = list_node.walk();
            for child in list_node.children(&mut child_cursor) {
                if child.kind() == "string" {
                    if let Ok(text) = child.utf8_text(source_bytes) {
                        let name = text.trim_matches('\'').trim_matches('"').to_string();
                        if !name.is_empty() && seen.insert(name.clone()) {
                            exports.push(name);
                        }
                    }
                }
            }
            exports.sort();
            return Some(exports);
        }
        None
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        for query in &self.import_queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let root_module = text.split('.').next().unwrap_or(text).to_string();
                        if seen.insert(root_module.clone()) {
                            imports.push(root_module);
                        }
                    }
                }
            }
        }

        // from foo import bar
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.from_import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if !text.starts_with('.') {
                        let root_module = text.split('.').next().unwrap_or(text).to_string();
                        if seen.insert(root_module.clone()) {
                            imports.push(root_module);
                        }
                    }
                }
            }
        }

        imports.sort();
        imports
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.relative_import_query, root_node, source.as_bytes())
    }

    fn extract_decorators(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let simple = collect_matches(&self.decorator_query, root_node, source_bytes);
        let dotted = collect_matches(&self.dotted_decorator_query, root_node, source_bytes);

        let seen: HashSet<String> = simple.iter().cloned().collect();
        let mut merged = simple;
        merged.extend(dotted.into_iter().filter(|d| !seen.contains(d)));
        merged.sort();
        merged
    }
}

impl Parser for PythonParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python source"))?;

        let root_node = tree.root_node();

        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        let decorators = self.extract_decorators(source, root_node);
        let custom_fields = if decorators.is_empty() {
            None
        } else {
            let mut fields = HashMap::new();
            fields.insert(
                "decorators".to_string(),
                serde_json::Value::Array(
                    decorators
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
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_python_functions() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def hello():\n    pass\n\ndef world():\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"hello".to_string()));
        assert!(result.metadata.exports.contains(&"world".to_string()));
        assert_eq!(result.metadata.loc, 5);
    }

    #[test]
    fn parse_python_classes() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class MyClass:\n    pass\n\nclass _Private:\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.contains(&"MyClass".to_string()));
        assert!(!result.metadata.exports.contains(&"_Private".to_string()));
    }

    #[test]
    fn parse_python_imports() {
        let mut parser = PythonParser::new().unwrap();
        let source =
            "import os\nimport json\nfrom pathlib import Path\nfrom .utils import helper\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"os".to_string()));
        assert!(result.metadata.imports.contains(&"json".to_string()));
        assert!(result.metadata.imports.contains(&"pathlib".to_string()));
        assert!(!result.metadata.imports.contains(&".utils".to_string()));
    }

    #[test]
    fn parse_python_relative_deps() {
        let mut parser = PythonParser::new().unwrap();
        let source = "from .utils import helper\nfrom ..models import User\n";
        let result = parser.parse(source).unwrap();
        assert!(!result.metadata.dependencies.is_empty());
    }

    #[test]
    fn parse_python_private_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def _private():\n    pass\n\ndef public():\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(!result.metadata.exports.contains(&"_private".to_string()));
        assert!(result.metadata.exports.contains(&"public".to_string()));
    }

    #[test]
    fn python_custom_fields_decorators() {
        let mut parser = PythonParser::new().unwrap();
        let source =
            "@staticmethod\ndef foo():\n    pass\n\n@property\ndef bar(self):\n    return 1\n";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let decorators = fields.get("decorators").unwrap().as_array().unwrap();
        let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"staticmethod"));
        assert!(names.contains(&"property"));
    }

    #[test]
    fn python_no_custom_fields_when_no_decorators() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def foo():\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(result.custom_fields.is_none());
    }

    #[test]
    fn parse_python_dunder_all() {
        let mut parser = PythonParser::new().unwrap();
        let source = r#"
__all__ = ["public_func", "PublicClass"]

def public_func():
    pass

def _private_func():
    pass

class PublicClass:
    pass

class _InternalClass:
    pass
"#;
        let result = parser.parse(source).unwrap();
        assert_eq!(result.metadata.exports, vec!["PublicClass", "public_func"]);
    }

    #[test]
    fn parse_python_aliased_import() {
        let mut parser = PythonParser::new().unwrap();
        let source = "import pandas as pd\nimport numpy as np\nimport os\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"pandas".to_string()));
        assert!(result.metadata.imports.contains(&"numpy".to_string()));
        assert!(result.metadata.imports.contains(&"os".to_string()));
    }

    #[test]
    fn parse_python_dunder_all_overrides_discovery() {
        let mut parser = PythonParser::new().unwrap();
        let source = r#"
__all__ = ["only_this"]

def only_this():
    pass

def also_public():
    pass
"#;
        let result = parser.parse(source).unwrap();
        assert_eq!(result.metadata.exports, vec!["only_this"]);
        assert!(!result.metadata.exports.contains(&"also_public".to_string()));
    }
}
