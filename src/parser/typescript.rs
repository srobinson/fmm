use super::{Metadata, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct TypeScriptParser {
    parser: TSParser,
    language: Language,
}

impl TypeScriptParser {
    pub fn new() -> Result<Self> {
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set language: {}", e))?;

        Ok(Self { parser, language })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        // Query for various export patterns
        let queries = [
            // export function foo() {}
            "(export_statement (function_declaration name: (identifier) @name))",
            // export const foo = ...
            "(export_statement (lexical_declaration (variable_declarator name: (identifier) @name)))",
            // export class Foo {}
            "(export_statement (class_declaration name: (type_identifier) @name))",
            // export interface Foo {}
            "(export_statement (interface_declaration name: (type_identifier) @name))",
            // export { foo }
            "(export_statement (export_clause (export_specifier name: (identifier) @name)))",
        ];

        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        for query_str in queries {
            if let Ok(query) = Query::new(&self.language, query_str) {
                let mut cursor = QueryCursor::new();
                let mut iter = cursor.matches(&query, root_node, source_bytes);

                while let Some(m) = iter.next() {
                    for capture in m.captures {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            let text_string = text.to_string();
                            if !exports.contains(&text_string) {
                                exports.push(text_string);
                            }
                        }
                    }
                }
            }
        }

        // Sort and deduplicate
        exports.sort();
        exports.dedup();
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        // Query for import statements
        let query_str = r#"
            (import_statement source: (string) @source)
        "#;

        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        if let Ok(query) = Query::new(&self.language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);

            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        // Remove quotes
                        let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                        if !imports.contains(&cleaned) {
                            imports.push(cleaned);
                        }
                    }
                }
            }
        }

        // External imports only (npm packages)
        imports
            .into_iter()
            .filter(|imp| !imp.starts_with('.') && !imp.starts_with('/'))
            .collect()
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let query_str = r#"
            (import_statement source: (string) @source)
        "#;

        let mut dependencies = Vec::new();
        let source_bytes = source.as_bytes();

        if let Ok(query) = Query::new(&self.language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);

            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                        // Only local dependencies (relative imports)
                        if (cleaned.starts_with('.') || cleaned.starts_with('/'))
                            && !dependencies.contains(&cleaned)
                        {
                            dependencies.push(cleaned);
                        }
                    }
                }
            }
        }

        dependencies.sort();
        dependencies.dedup();
        dependencies
    }
}

impl Parser for TypeScriptParser {
    fn parse(&mut self, source: &str) -> Result<Metadata> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source code"))?;

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
}
