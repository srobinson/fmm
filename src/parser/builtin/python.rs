use super::query_helpers::{collect_matches, compile_query, make_parser, top_level_ancestor};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser as TSParser, Query, QueryCursor};

/// Convert Python relative import dot-notation to path notation so that
/// `dep_matches()` can resolve it against manifest file paths.
///
/// Examples:
/// - `.utils`    → `./utils`
/// - `..models`  → `../models`
/// - `...deep.sub` → `../../deep/sub`
/// - `.`         → `./` (bare relative import, e.g. `from . import X`)
fn dot_import_to_path(raw: &str) -> String {
    let dots = raw.chars().take_while(|c| *c == '.').count();
    let rest = &raw[dots..];
    let module_path = rest.replace('.', "/");
    if dots <= 1 {
        format!("./{}", module_path)
    } else {
        let ups = "../".repeat(dots - 1);
        format!("{}{}", ups, module_path)
    }
}

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
    /// ALP-769: finds class declarations for public method extraction
    class_method_query: Query,
}

impl PythonParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let parser = make_parser(&language, "Python")?;

        let func_query = compile_query(
            &language,
            "[(module (function_definition name: (identifier) @name))
              (module (decorated_definition (function_definition name: (identifier) @name)))]",
            "func",
        )?;
        let class_query = compile_query(
            &language,
            "[(module (class_definition name: (identifier) @name))
              (module (decorated_definition (class_definition name: (identifier) @name)))]",
            "class",
        )?;
        let assign_query = compile_query(
            &language,
            "(module (expression_statement (assignment left: (identifier) @name)))",
            "assign",
        )?;
        let dunder_all_query = compile_query(
            &language,
            "(module (expression_statement (assignment left: (identifier) @name right: (list) @values)))",
            "dunder_all",
        )?;
        let import_queries = vec![
            compile_query(
                &language,
                "(import_statement name: (dotted_name) @name)",
                "import",
            )?,
            compile_query(
                &language,
                "(import_statement name: (aliased_import name: (dotted_name) @name))",
                "aliased import",
            )?,
        ];
        let from_import_query = compile_query(
            &language,
            "(import_from_statement module_name: (dotted_name) @name)",
            "from_import",
        )?;
        let relative_import_query = compile_query(
            &language,
            "(import_from_statement module_name: (relative_import) @name)",
            "relative_import",
        )?;
        let decorator_query =
            compile_query(&language, "(decorator (identifier) @name)", "decorator")?;
        let dotted_decorator_query = compile_query(
            &language,
            "(decorator (attribute) @name)",
            "dotted_decorator",
        )?;
        // ALP-769: find class declarations for public method extraction
        let class_method_query = compile_query(
            &language,
            "(class_definition name: (identifier) @class_name) @class",
            "class_method",
        )?;

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
            class_method_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
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
                            let decl = top_level_ancestor(capture.node);
                            exports.push(ExportEntry::new(
                                text.to_string(),
                                decl.start_position().row + 1,
                                decl.end_position().row + 1,
                            ));
                        }
                    }
                }
            }
        };

        collect_filtered(&self.func_query, |_| true);
        collect_filtered(&self.class_query, |_| true);
        collect_filtered(&self.assign_query, |name| {
            name.chars().all(|c| c.is_uppercase() || c == '_')
                || name.chars().next().is_some_and(|c| c.is_uppercase())
        });

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    /// Build a map of top-level definition names to their line ranges.
    fn build_definition_map(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> HashMap<String, (usize, usize)> {
        let source_bytes = source.as_bytes();
        let mut defs = HashMap::new();

        let mut collect_defs = |query: &Query| {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let decl = top_level_ancestor(capture.node);
                        defs.insert(
                            text.to_string(),
                            (decl.start_position().row + 1, decl.end_position().row + 1),
                        );
                    }
                }
            }
        };

        collect_defs(&self.func_query);
        collect_defs(&self.class_query);
        collect_defs(&self.assign_query);
        defs
    }

    /// Extract names from `__all__ = [...]` if present, resolving to definition sites.
    fn extract_dunder_all(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Option<Vec<ExportEntry>> {
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

            // Build definition map to resolve names to their actual definition sites
            let def_map = self.build_definition_map(source, root_node);

            let all_node = top_level_ancestor(name_capture.node);
            let all_start = all_node.start_position().row + 1;
            let all_end = all_node.end_position().row + 1;

            let mut seen = HashSet::new();
            let mut exports = Vec::new();
            let list_node = values_capture.node;
            let mut child_cursor = list_node.walk();
            for child in list_node.children(&mut child_cursor) {
                if child.kind() == "string" {
                    if let Ok(text) = child.utf8_text(source_bytes) {
                        let name = text.trim_matches('\'').trim_matches('"').to_string();
                        if !name.is_empty() && seen.insert(name.clone()) {
                            let (start, end) =
                                def_map.get(&name).copied().unwrap_or((all_start, all_end));
                            exports.push(ExportEntry::new(name, start, end));
                        }
                    }
                }
            }
            exports.sort_by_key(|e| e.start_line);
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
                        let full_module = text.to_string();
                        if seen.insert(full_module.clone()) {
                            imports.push(full_module);
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
                        let full_module = text.to_string();
                        if seen.insert(full_module.clone()) {
                            imports.push(full_module);
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
            .into_iter()
            .map(|s| dot_import_to_path(&s))
            .collect()
    }

    /// ALP-769: extract public methods from exported classes.
    /// Returns `ExportEntry` items with `parent_class` set to the class name.
    ///
    /// Public heuristic: include if name does not start with `_`, OR name is `__init__`.
    /// All other dunder methods (`__str__`, `__repr__`, etc.) are skipped.
    /// Decorated methods (`@property`, `@staticmethod`, etc.) are included.
    fn extract_class_methods(
        &self,
        source: &str,
        root_node: Node,
        exported_class_names: &HashSet<String>,
    ) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut entries = Vec::new();

        let class_name_idx = self
            .class_method_query
            .capture_index_for_name("class_name")
            .unwrap_or(0);
        let class_idx = self
            .class_method_query
            .capture_index_for_name("class")
            .unwrap_or(1);

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.class_method_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            let mut class_node: Option<Node> = None;
            let mut class_name: Option<String> = None;

            for cap in m.captures {
                if cap.index == class_name_idx {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        class_name = Some(text.to_string());
                    }
                } else if cap.index == class_idx {
                    class_node = Some(cap.node);
                }
            }

            let (class_node, class_name) = match (class_node, class_name) {
                (Some(n), Some(name)) => (n, name),
                _ => continue,
            };

            if !exported_class_names.contains(&class_name) {
                continue;
            }

            let body = match class_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            for i in 0..body.child_count() {
                if let Some(child) = body.child(i as u32) {
                    match child.kind() {
                        "function_definition" => {
                            if let Some(entry) =
                                Self::extract_python_method_entry(&class_name, child, source_bytes)
                            {
                                entries.push(entry);
                            }
                        }
                        "decorated_definition" => {
                            // Find the function_definition inside the decorated_definition
                            for j in 0..child.child_count() {
                                if let Some(inner) = child.child(j as u32) {
                                    if inner.kind() == "function_definition" {
                                        if let Some(mut entry) = Self::extract_python_method_entry(
                                            &class_name,
                                            inner,
                                            source_bytes,
                                        ) {
                                            // Use the decorated_definition range to include decorator lines
                                            entry.start_line = child.start_position().row + 1;
                                            entry.end_line = child.end_position().row + 1;
                                            entries.push(entry);
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        entries
    }

    /// Extract a single function_definition node as an ExportEntry.
    /// Returns None for private methods (leading underscore), except `__init__`.
    fn extract_python_method_entry(
        class_name: &str,
        method_node: Node,
        source_bytes: &[u8],
    ) -> Option<ExportEntry> {
        let name_node = method_node.child_by_field_name("name")?;
        let method_name = name_node.utf8_text(source_bytes).ok()?.to_string();

        // Include public methods and __init__; skip all other underscore-prefixed names
        if method_name.starts_with('_') && method_name != "__init__" {
            return None;
        }

        Some(ExportEntry::method(
            method_name,
            method_node.start_position().row + 1,
            method_node.end_position().row + 1,
            class_name.to_string(),
        ))
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

        let mut exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        // ALP-769: extract public methods from exported classes
        let exported_classes: HashSet<String> = exports
            .iter()
            .filter(|e| e.parent_class.is_none())
            .map(|e| e.name.clone())
            .collect();
        let methods = self.extract_class_methods(source, root_node, &exported_classes);
        exports.extend(methods);
        exports.sort_by_key(|e| e.start_line);

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
                ..Default::default()
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

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "python",
        extensions: &["py"],
        reexport_filenames: &["__init__.py"],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &["_test.py"],
            filename_prefixes: &["test_"],
            test_symbol_prefixes: &["test_"],
        },
    };

impl crate::parser::LanguageDescriptor for PythonParser {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn reexport_filenames(&self) -> &'static [&'static str] {
        &["__init__.py"]
    }

    fn test_file_patterns(&self) -> crate::parser::LanguageTestPatterns {
        crate::parser::LanguageTestPatterns {
            filename_suffixes: &["_test.py"],
            filename_prefixes: &["test_"],
            test_symbol_prefixes: &["test_"],
        }
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
        assert!(result
            .metadata
            .export_names()
            .contains(&"hello".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"world".to_string()));
        assert_eq!(result.metadata.loc, 5);
    }

    #[test]
    fn parse_python_classes() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class MyClass:\n    pass\n\nclass _Private:\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"MyClass".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"_Private".to_string()));
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
        let deps = &result.metadata.dependencies;
        assert!(
            deps.contains(&"./utils".to_string()),
            "expected ./utils in {:?}",
            deps
        );
        assert!(
            deps.contains(&"../models".to_string()),
            "expected ../models in {:?}",
            deps
        );
    }

    #[test]
    fn dot_import_to_path_conversions() {
        assert_eq!(dot_import_to_path(".utils"), "./utils");
        assert_eq!(dot_import_to_path("..models"), "../models");
        assert_eq!(dot_import_to_path("...deep.sub"), "../../deep/sub");
        assert_eq!(dot_import_to_path("."), "./");
    }

    #[test]
    fn parse_python_private_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "def _private():\n    pass\n\ndef public():\n    pass\n";
        let result = parser.parse(source).unwrap();
        assert!(!result
            .metadata
            .export_names()
            .contains(&"_private".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"public".to_string()));
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
        assert_eq!(
            result.metadata.export_names(),
            vec!["public_func", "PublicClass"]
        );
        // Verify exports resolve to definition sites, not __all__ line
        let exports = &result.metadata.exports;
        let func_export = exports.iter().find(|e| e.name == "public_func").unwrap();
        assert_eq!(func_export.start_line, 4);
        assert_eq!(func_export.end_line, 5);
        let class_export = exports.iter().find(|e| e.name == "PublicClass").unwrap();
        assert_eq!(class_export.start_line, 10);
        assert_eq!(class_export.end_line, 11);
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
    fn parse_python_decorated_class() {
        let mut parser = PythonParser::new().unwrap();
        let source =
            "from dataclasses import dataclass\n\n@dataclass\nclass Agent:\n    name: str\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Agent".to_string()));
    }

    #[test]
    fn parse_python_decorated_class_with_args() {
        let mut parser = PythonParser::new().unwrap();
        let source = "@dataclass(frozen=True)\nclass Config:\n    debug: bool = False\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Config".to_string()));
    }

    #[test]
    fn parse_python_decorated_function() {
        let mut parser = PythonParser::new().unwrap();
        let source = "from flask import Flask\napp = Flask(__name__)\n\n@app.route(\"/\")\ndef handler():\n    return \"ok\"\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"handler".to_string()));
    }

    #[test]
    fn parse_python_decorated_class_line_range() {
        let mut parser = PythonParser::new().unwrap();
        let source = "@dataclass\nclass Agent:\n    name: str\n    role: str\n";
        let result = parser.parse(source).unwrap();
        let agent = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Agent")
            .expect("Agent should be exported");
        // Range should start at the decorator line (1), not the class line (2)
        assert_eq!(agent.start_line, 1);
        assert_eq!(agent.end_line, 4);
    }

    #[test]
    fn parse_python_dunder_all_with_decorated_class() {
        let mut parser = PythonParser::new().unwrap();
        let source = r#"
from dataclasses import dataclass

__all__ = ["DecoratedModel", "bare_func"]

@dataclass
class DecoratedModel:
    id: int
    name: str

def bare_func():
    pass
"#;
        let result = parser.parse(source).unwrap();
        assert_eq!(
            result.metadata.export_names(),
            vec!["DecoratedModel", "bare_func"]
        );
        // DecoratedModel should resolve to the decorated_definition site, not __all__ line
        let model = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "DecoratedModel")
            .unwrap();
        assert_eq!(model.start_line, 6); // @dataclass line
        assert_eq!(model.end_line, 9);
    }

    #[test]
    fn parse_python_dotted_imports_full_path() {
        // `from agno.models.message import Message` should store "agno.models.message",
        // not just the root "agno". Single-name imports are unaffected.
        let mut parser = PythonParser::new().unwrap();
        let source = "from agno.models.message import Message\nfrom agno.tools.function import Function\nimport os\n";
        let result = parser.parse(source).unwrap();
        assert!(
            result
                .metadata
                .imports
                .contains(&"agno.models.message".to_string()),
            "expected full dotted path, got: {:?}",
            result.metadata.imports
        );
        assert!(
            result
                .metadata
                .imports
                .contains(&"agno.tools.function".to_string()),
            "expected full dotted path, got: {:?}",
            result.metadata.imports
        );
        // Single-name import unchanged
        assert!(result.metadata.imports.contains(&"os".to_string()));
        // Deduplicated: only one entry per unique dotted path
        assert_eq!(
            result
                .metadata
                .imports
                .iter()
                .filter(|i| i.as_str() == "agno.models.message")
                .count(),
            1
        );
    }

    // ALP-769: public method extraction tests

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
    fn python_methods_public_included() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class Foo:\n    def bar(self):\n        pass\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "bar").is_some(),
            "Foo.bar should be indexed"
        );
    }

    #[test]
    fn python_methods_private_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class Foo:\n    def _internal(self):\n        pass\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "_internal").is_none(),
            "Foo._internal should NOT be indexed"
        );
    }

    #[test]
    fn python_methods_init_included() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class Foo:\n    def __init__(self):\n        pass\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "__init__").is_some(),
            "Foo.__init__ should be indexed"
        );
    }

    #[test]
    fn python_methods_other_dunder_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class Foo:\n    def __str__(self):\n        return ''\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "__str__").is_none(),
            "Foo.__str__ should NOT be indexed"
        );
    }

    #[test]
    fn python_methods_non_exported_class_excluded() {
        let mut parser = PythonParser::new().unwrap();
        let source = "class _Internal:\n    def method(self):\n        pass\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "_Internal", "method").is_none(),
            "methods of non-exported class should NOT be indexed"
        );
    }

    #[test]
    fn python_methods_decorated_included() {
        let mut parser = PythonParser::new().unwrap();
        let source =
            "class Foo:\n    @property\n    def value(self):\n        return self._value\n    @staticmethod\n    def create():\n        return Foo()\n";
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "Foo", "value").is_some(),
            "Foo.value (@property) should be indexed"
        );
        assert!(
            get_method(&result.metadata.exports, "Foo", "create").is_some(),
            "Foo.create (@staticmethod) should be indexed"
        );
    }

    #[test]
    fn python_methods_decorated_line_range_includes_decorator() {
        let mut parser = PythonParser::new().unwrap();
        // line 1: class Foo:
        // line 2:     @property
        // line 3:     def value(self):
        // line 4:         return 1
        let source = "class Foo:\n    @property\n    def value(self):\n        return 1\n";
        let result = parser.parse(source).unwrap();
        let entry = get_method(&result.metadata.exports, "Foo", "value")
            .expect("Foo.value should be indexed");
        assert_eq!(
            entry.start_line, 2,
            "start_line should be the decorator line"
        );
    }

    #[test]
    fn python_methods_dunder_all_respects_export_list() {
        let mut parser = PythonParser::new().unwrap();
        let source = r#"
__all__ = ["PublicClass"]

class PublicClass:
    def method(self):
        pass

class HiddenClass:
    def method(self):
        pass
"#;
        let result = parser.parse(source).unwrap();
        assert!(
            get_method(&result.metadata.exports, "PublicClass", "method").is_some(),
            "PublicClass.method should be indexed"
        );
        assert!(
            get_method(&result.metadata.exports, "HiddenClass", "method").is_none(),
            "HiddenClass.method should NOT be indexed (not in __all__)"
        );
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
        assert_eq!(result.metadata.export_names(), vec!["only_this"]);
        assert!(!result
            .metadata
            .export_names()
            .contains(&"also_public".to_string()));
    }
}
