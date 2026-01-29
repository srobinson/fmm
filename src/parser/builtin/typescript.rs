use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct TypeScriptParser {
    parser: TSParser,
    export_queries: Vec<Query>,
    import_query: Query,
}

impl TypeScriptParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set language: {}", e))?;

        let export_query_strs = [
            "(export_statement (function_declaration name: (identifier) @name))",
            "(export_statement (lexical_declaration (variable_declarator name: (identifier) @name)))",
            "(export_statement (class_declaration name: (type_identifier) @name))",
            "(export_statement (interface_declaration name: (type_identifier) @name))",
            "(export_statement (export_clause (export_specifier name: (identifier) @name)))",
        ];

        let export_queries: Vec<Query> = export_query_strs
            .iter()
            .map(|q| Query::new(&language, q))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to compile export query: {}", e))?;

        let import_query = Query::new(&language, r#"(import_statement source: (string) @source)"#)
            .map_err(|e| anyhow::anyhow!("Failed to compile import query: {}", e))?;

        Ok(Self {
            parser,
            export_queries,
            import_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        for query in &self.export_queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);

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

        exports.sort();
        exports.dedup();
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if !imports.contains(&cleaned) {
                        imports.push(cleaned);
                    }
                }
            }
        }

        imports
            .into_iter()
            .filter(|imp| !imp.starts_with('.') && !imp.starts_with('/'))
            .collect()
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut dependencies = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if (cleaned.starts_with('.') || cleaned.starts_with('/'))
                        && !dependencies.contains(&cleaned)
                    {
                        dependencies.push(cleaned);
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
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source code"))?;

        let root_node = tree.root_node();

        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
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
        "typescript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "js", "jsx"]
    }
}
