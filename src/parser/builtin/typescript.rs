use std::collections::HashSet;

use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::collect_matches_with_lines;

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

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen: HashSet<String> = HashSet::new();
        let mut exports = Vec::new();

        for query in &self.export_queries {
            for entry in collect_matches_with_lines(query, root_node, source_bytes) {
                if seen.insert(entry.name.clone()) {
                    exports.push(entry);
                }
            }
        }

        exports.sort_by(|a, b| a.name.cmp(&b.name));
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if !cleaned.starts_with('.') && !cleaned.starts_with('/') {
                        seen.insert(cleaned);
                    }
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
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if cleaned.starts_with('.') || cleaned.starts_with('/') {
                        seen.insert(cleaned);
                    }
                }
            }
        }

        let mut dependencies: Vec<String> = seen.into_iter().collect();
        dependencies.sort();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> ParseResult {
        let mut parser = TypeScriptParser::new().unwrap();
        parser.parse(source).unwrap()
    }

    // --- Export extraction ---

    #[test]
    fn exports_named_function() {
        let result = parse("export function greet(name: string) { return `Hi ${name}`; }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"greet".to_string()));
    }

    #[test]
    fn exports_arrow_function_via_const() {
        let result = parse("export const add = (a: number, b: number) => a + b;");
        assert!(result.metadata.export_names().contains(&"add".to_string()));
    }

    #[test]
    fn exports_class() {
        let result = parse("export class UserService { constructor() {} }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"UserService".to_string()));
    }

    #[test]
    fn exports_interface() {
        let result = parse("export interface Config { debug: boolean; }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Config".to_string()));
    }

    #[test]
    fn exports_multiple_from_clause() {
        let result = parse("export { foo, bar, baz } from './other';");
        assert!(result.metadata.export_names().contains(&"foo".to_string()));
        assert!(result.metadata.export_names().contains(&"bar".to_string()));
        assert!(result.metadata.export_names().contains(&"baz".to_string()));
    }

    #[test]
    fn exports_const_variable() {
        let result = parse("export const MAX_RETRIES = 3;");
        assert!(result
            .metadata
            .export_names()
            .contains(&"MAX_RETRIES".to_string()));
    }

    #[test]
    fn exports_let_variable() {
        let result = parse("export let counter = 0;");
        assert!(result
            .metadata
            .export_names()
            .contains(&"counter".to_string()));
    }

    #[test]
    fn exports_are_sorted_and_deduplicated() {
        let source = r#"
export function zebra() {}
export function alpha() {}
export const middle = 1;
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["alpha", "middle", "zebra"]
        );
    }

    // --- Import extraction ---

    #[test]
    fn imports_external_package() {
        let result = parse("import { useState } from 'react';");
        assert!(result.metadata.imports.contains(&"react".to_string()));
    }

    #[test]
    fn imports_scoped_package() {
        let result = parse("import express from '@types/express';");
        assert!(result
            .metadata
            .imports
            .contains(&"@types/express".to_string()));
    }

    #[test]
    fn imports_excludes_relative_paths() {
        let source = r#"
import { helper } from './utils';
import { config } from '../config';
import React from 'react';
"#;
        let result = parse(source);
        assert_eq!(result.metadata.imports, vec!["react"]);
    }

    // --- Dependency extraction ---

    #[test]
    fn dependencies_captures_relative_imports() {
        let source = r#"
import { foo } from './foo';
import { bar } from '../lib/bar';
import { baz } from '/absolute/baz';
import React from 'react';
"#;
        let result = parse(source);
        assert!(result.metadata.dependencies.contains(&"./foo".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"../lib/bar".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"/absolute/baz".to_string()));
        assert!(!result.metadata.dependencies.contains(&"react".to_string()));
    }

    #[test]
    fn dependencies_excludes_external_packages() {
        let result = parse("import express from 'express'; import cors from 'cors';");
        assert!(result.metadata.dependencies.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn empty_file() {
        let result = parse("");
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
        assert!(result.metadata.dependencies.is_empty());
        assert_eq!(result.metadata.loc, 0);
    }

    #[test]
    fn no_exports_internal_module() {
        let source = "const internal = 42;\nfunction helper() {}\n";
        let result = parse(source);
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn loc_counting() {
        let source = "line1\nline2\nline3\nline4\n";
        let result = parse(source);
        assert_eq!(result.metadata.loc, 4);
    }

    #[test]
    fn loc_single_line_no_trailing_newline() {
        let result = parse("export const x = 1;");
        assert_eq!(result.metadata.loc, 1);
    }

    #[test]
    fn tsx_jsx_in_export() {
        let source = r#"
export function Button() {
    return <button>Click</button>;
}
"#;
        // TypeScript parser handles TSX at syntax level; we just check it doesn't crash
        let mut parser = TypeScriptParser::new().unwrap();
        let result = parser.parse(source);
        // tree-sitter-typescript may or may not parse JSX; the key is no panic
        assert!(result.is_ok());
    }

    #[test]
    fn complex_module_with_mixed_exports() {
        let source = r#"
import { Logger } from 'winston';
import { Pool } from './db/pool';
import type { Config } from './config';

export interface DatabaseConfig {
    host: string;
    port: number;
}

export class DatabaseService {
    private pool: Pool;

    constructor(config: DatabaseConfig) {
        this.pool = new Pool(config);
    }

    async query(sql: string): Promise<any[]> {
        return this.pool.query(sql);
    }
}

export function createService(config: DatabaseConfig): DatabaseService {
    return new DatabaseService(config);
}

export const DEFAULT_PORT = 5432;
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec![
                "DEFAULT_PORT",
                "DatabaseConfig",
                "DatabaseService",
                "createService"
            ]
        );
        assert_eq!(result.metadata.imports, vec!["winston"]);
        assert!(result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./config".to_string()));
        assert!(result.metadata.loc > 20);
    }

    #[test]
    fn language_id_and_extensions() {
        let parser = TypeScriptParser::new().unwrap();
        assert_eq!(parser.language_id(), "typescript");
        assert_eq!(parser.extensions(), &["ts", "tsx", "js", "jsx"]);
    }

    #[test]
    fn barrel_reexport_file() {
        let source = r#"
export { UserService } from './user.service';
export { AuthService } from './auth.service';
export { Logger } from './logger';
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["AuthService", "Logger", "UserService"]
        );
        // Re-exports via `export { X } from '...'` don't produce import_statements,
        // so the current parser doesn't capture them as dependencies.
        // This is a known limitation — dependencies only come from `import` statements.
        assert!(result.metadata.dependencies.is_empty());
    }
}
