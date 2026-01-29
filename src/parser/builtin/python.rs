use crate::parser::{Metadata, Parser};
use anyhow::Result;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct PythonParser {
    parser: TSParser,
    language: Language,
}

impl PythonParser {
    pub fn new() -> Result<Self> {
        let language = tree_sitter_python::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Python language: {}", e))?;

        Ok(Self { parser, language })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        // Top-level function definitions
        let func_query = "(module (function_definition name: (identifier) @name))";
        if let Ok(query) = Query::new(&self.language, func_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        if !name.starts_with('_') && !exports.contains(&name) {
                            exports.push(name);
                        }
                    }
                }
            }
        }

        // Top-level class definitions
        let class_query = "(module (class_definition name: (identifier) @name))";
        if let Ok(query) = Query::new(&self.language, class_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        if !name.starts_with('_') && !exports.contains(&name) {
                            exports.push(name);
                        }
                    }
                }
            }
        }

        // Top-level assignments (module-level constants)
        let assign_query = "(module (expression_statement (assignment left: (identifier) @name)))";
        if let Ok(query) = Query::new(&self.language, assign_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        // Only UPPER_CASE constants or public names
                        if !name.starts_with('_')
                            && (name.chars().all(|c| c.is_uppercase() || c == '_')
                                || name.chars().next().is_some_and(|c| c.is_uppercase()))
                            && !exports.contains(&name)
                        {
                            exports.push(name);
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

        // import foo
        let import_query = "(import_statement name: (dotted_name) @name)";
        if let Ok(query) = Query::new(&self.language, import_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let root_module = text.split('.').next().unwrap_or(text).to_string();
                        if !imports.contains(&root_module) {
                            imports.push(root_module);
                        }
                    }
                }
            }
        }

        // from foo import bar
        let from_query = "(import_from_statement module_name: (dotted_name) @name)";
        if let Ok(query) = Query::new(&self.language, from_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let module = text.to_string();
                        // Skip relative imports (they're dependencies)
                        if !module.starts_with('.') {
                            let root_module =
                                module.split('.').next().unwrap_or(&module).to_string();
                            if !imports.contains(&root_module) {
                                imports.push(root_module);
                            }
                        }
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

        // from .foo import bar (relative imports)
        let from_query = "(import_from_statement module_name: (relative_import) @name)";
        if let Ok(query) = Query::new(&self.language, from_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let dep = text.to_string();
                        if !deps.contains(&dep) {
                            deps.push(dep);
                        }
                    }
                }
            }
        }

        deps.sort();
        deps.dedup();
        deps
    }

    fn extract_decorators(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut decorators = Vec::new();
        let source_bytes = source.as_bytes();

        let query_str = "(decorator (identifier) @name)";
        if let Ok(query) = Query::new(&self.language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        if !decorators.contains(&name) {
                            decorators.push(name);
                        }
                    }
                }
            }
        }

        // Also handle dotted decorators like @app.route
        let dotted_query = "(decorator (attribute) @name)";
        if let Ok(query) = Query::new(&self.language, dotted_query) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        if !decorators.contains(&name) {
                            decorators.push(name);
                        }
                    }
                }
            }
        }

        decorators.sort();
        decorators
    }
}

impl Parser for PythonParser {
    fn parse(&mut self, source: &str) -> Result<Metadata> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python source"))?;

        let root_node = tree.root_node();

        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        Ok(Metadata {
            exports,
            imports,
            dependencies,
            loc,
        })
    }

    fn language_id(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn custom_fields(&self, source: &str) -> Option<HashMap<String, serde_json::Value>> {
        // Re-parse to get decorators (we don't cache the tree for thread safety)
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let mut parser = TSParser::new();
        if parser.set_language(&language).is_err() {
            return None;
        }
        let tree = parser.parse(source, None)?;
        let root_node = tree.root_node();

        let decorators = self.extract_decorators(source, root_node);
        if decorators.is_empty() {
            return None;
        }

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_python_functions() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def hello():\n    pass\n\ndef world():\n    pass\n";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.exports.contains(&"hello".to_string()));
        assert!(metadata.exports.contains(&"world".to_string()));
        assert_eq!(metadata.loc, 5);
    }

    #[test]
    fn parse_python_classes() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class MyClass:\n    pass\n\nclass _Private:\n    pass\n";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.exports.contains(&"MyClass".to_string()));
        assert!(!metadata.exports.contains(&"_Private".to_string()));
    }

    #[test]
    fn parse_python_imports() {
        let mut parser = PythonParser::new().unwrap();
        let source =
            "import os\nimport json\nfrom pathlib import Path\nfrom .utils import helper\n";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.imports.contains(&"os".to_string()));
        assert!(metadata.imports.contains(&"json".to_string()));
        assert!(metadata.imports.contains(&"pathlib".to_string()));
        // Relative imports should be in dependencies, not imports
        assert!(!metadata.imports.contains(&".utils".to_string()));
    }

    #[test]
    fn parse_python_relative_deps() {
        let mut parser = PythonParser::new().unwrap();
        let source = "from .utils import helper\nfrom ..models import User\n";
        let metadata = parser.parse(source).unwrap();
        assert!(!metadata.dependencies.is_empty());
    }

    #[test]
    fn parse_python_private_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def _private():\n    pass\n\ndef public():\n    pass\n";
        let metadata = parser.parse(source).unwrap();
        assert!(!metadata.exports.contains(&"_private".to_string()));
        assert!(metadata.exports.contains(&"public".to_string()));
    }

    #[test]
    fn python_custom_fields_decorators() {
        let parser = PythonParser::new().unwrap();
        let source =
            "@staticmethod\ndef foo():\n    pass\n\n@property\ndef bar(self):\n    return 1\n";
        let fields = parser.custom_fields(source);
        assert!(fields.is_some());
        let fields = fields.unwrap();
        let decorators = fields.get("decorators").unwrap().as_array().unwrap();
        let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"staticmethod"));
        assert!(names.contains(&"property"));
    }

    #[test]
    fn python_no_custom_fields_when_no_decorators() {
        let parser = PythonParser::new().unwrap();
        let source = "def foo():\n    pass\n";
        assert!(parser.custom_fields(source).is_none());
    }
}
