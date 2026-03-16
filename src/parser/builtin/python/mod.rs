mod extract_classes;

#[cfg(test)]
mod tests;

use super::query_helpers::{collect_matches, compile_query, make_parser, top_level_ancestor};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

/// Convert Python relative import dot-notation to path notation for `dep_matches()`.
/// `.utils` -> `./utils`, `..models` -> `../models`, `.` -> `./`
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
    pub(super) class_method_query: Query,
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
