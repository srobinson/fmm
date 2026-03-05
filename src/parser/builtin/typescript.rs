use std::collections::{HashMap, HashSet};

use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::collect_matches_with_lines;

pub struct TypeScriptParser {
    parser: TSParser,
    export_queries: Vec<Query>,
    import_query: Query,
    /// ALP-749/750: captures source string from `export { X } from '...'` and `export * from '...'`
    reexport_source_query: Query,
    /// ALP-754: `@Foo` style decorators
    decorator_query: Query,
    /// ALP-754: `@Foo()` call-expression style decorators
    call_decorator_query: Query,
    /// true when built for TSX/JSX (ALP-753)
    is_tsx: bool,
}

impl TypeScriptParser {
    /// Parser for `.ts` and `.js` files — uses `LANGUAGE_TYPESCRIPT`.
    pub fn new() -> Result<Self> {
        Self::build(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), false)
    }

    /// Parser for `.tsx` and `.jsx` files — uses `LANGUAGE_TSX` (ALP-753).
    pub fn new_tsx() -> Result<Self> {
        Self::build(tree_sitter_typescript::LANGUAGE_TSX.into(), true)
    }

    fn build(language: Language, is_tsx: bool) -> Result<Self> {
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set language: {}", e))?;

        let export_query_strs = [
            "(export_statement (function_declaration name: (identifier) @name))",
            "(export_statement (lexical_declaration (variable_declarator name: (identifier) @name)))",
            "(export_statement (class_declaration name: (type_identifier) @name))",
            "(export_statement (interface_declaration name: (type_identifier) @name))",
            // ALP-752: capture alias when present (export { foo as bar } → bar)
            "(export_statement (export_clause (export_specifier alias: (identifier) @name)))",
            // ALP-752: capture name only when no alias is present (export { foo } → foo)
            "(export_statement (export_clause (export_specifier !alias name: (identifier) @name)))",
            // export type Foo = { ... }
            "(export_statement (type_alias_declaration name: (type_identifier) @name))",
            // export default SomeIdentifier
            "(export_statement value: (identifier) @name)",
            // ALP-751: export enum Direction {} / export const enum Status {}
            "(export_statement (enum_declaration name: (identifier) @name))",
            // ALP-755: export * as ns from './mod'
            "(export_statement (namespace_export (identifier) @name))",
            // ALP-756: export namespace Foo {} (uses `internal_module` in tree-sitter-typescript)
            "(export_statement (internal_module name: (identifier) @name))",
            // ALP-756: export module Foo {} (uses `module` node)
            "(export_statement (module name: (identifier) @name))",
        ];

        let export_queries: Vec<Query> = export_query_strs
            .iter()
            .map(|q| Query::new(&language, q))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to compile export query: {}", e))?;

        let import_query = Query::new(&language, r#"(import_statement source: (string) @source)"#)
            .map_err(|e| anyhow::anyhow!("Failed to compile import query: {}", e))?;

        // ALP-749/750: captures `from '...'` on both `export { X } from` and `export * from`
        let reexport_source_query =
            Query::new(&language, r#"(export_statement source: (string) @source)"#)
                .map_err(|e| anyhow::anyhow!("Failed to compile reexport_source query: {}", e))?;

        // ALP-754: simple decorator `@Foo`
        let decorator_query = Query::new(&language, "(decorator (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile decorator query: {}", e))?;

        // ALP-754: call-expression decorator `@Foo(...)`
        let call_decorator_query = Query::new(
            &language,
            "(decorator (call_expression function: (identifier) @name))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile call_decorator query: {}", e))?;

        Ok(Self {
            parser,
            export_queries,
            import_query,
            reexport_source_query,
            decorator_query,
            call_decorator_query,
            is_tsx,
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

        exports.sort_by_key(|e| e.start_line);
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

        // Regular import statements
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

        // ALP-749/750: re-export sources — `export { X } from './y'` and `export * from './y'`
        let mut cursor2 = QueryCursor::new();
        let mut iter2 = cursor2.matches(&self.reexport_source_query, root_node, source_bytes);
        while let Some(m) = iter2.next() {
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

    /// ALP-754: extract unique decorator names from the file.
    fn extract_decorators(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut decorators = Vec::new();

        for query in [&self.decorator_query, &self.call_decorator_query] {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        if seen.insert(text.to_string()) {
                            decorators.push(text.to_string());
                        }
                    }
                }
            }
        }

        decorators.sort();
        decorators
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
        if self.is_tsx {
            "tsx"
        } else {
            "typescript"
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        if self.is_tsx {
            &["tsx", "jsx"]
        } else {
            &["ts", "js"]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> ParseResult {
        let mut parser = TypeScriptParser::new().unwrap();
        parser.parse(source).unwrap()
    }

    fn parse_tsx(source: &str) -> ParseResult {
        let mut parser = TypeScriptParser::new_tsx().unwrap();
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
    fn exports_are_sorted_by_line_and_deduplicated() {
        let source = r#"
export function zebra() {}
export function alpha() {}
export const middle = 1;
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["zebra", "alpha", "middle"]
        );
    }

    // --- ALP-751: Enum exports ---

    #[test]
    fn exports_enum() {
        let result = parse("export enum Direction { Up, Down, Left, Right }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Direction".to_string()));
    }

    #[test]
    fn exports_const_enum() {
        let result = parse("export const enum Status { Active, Inactive }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Status".to_string()));
    }

    #[test]
    fn exports_enum_line_range() {
        let source = "// header\nexport enum Color {\n    Red,\n    Green,\n    Blue,\n}\n";
        let result = parse(source);
        let entry = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Color")
            .unwrap();
        assert_eq!(entry.start_line, 2);
        assert_eq!(entry.end_line, 6);
    }

    // --- ALP-752: Renamed export specifiers ---

    #[test]
    fn exports_aliased_specifier_captures_alias() {
        let result = parse("export { foo as bar } from './mod';");
        assert!(result.metadata.export_names().contains(&"bar".to_string()));
        assert!(!result.metadata.export_names().contains(&"foo".to_string()));
    }

    #[test]
    fn exports_unaliased_specifier_unchanged() {
        let result = parse("export { foo } from './mod';");
        assert!(result.metadata.export_names().contains(&"foo".to_string()));
    }

    #[test]
    fn exports_mixed_aliased_and_unaliased() {
        let result = parse("export { a as b, c } from './mod';");
        assert!(result.metadata.export_names().contains(&"b".to_string()));
        assert!(result.metadata.export_names().contains(&"c".to_string()));
        assert!(!result.metadata.export_names().contains(&"a".to_string()));
    }

    #[test]
    fn exports_aliased_specifier_with_dep_capture() {
        let result = parse("export { foo as bar } from './mod';");
        assert!(result.metadata.dependencies.contains(&"./mod".to_string()));
    }

    // --- ALP-755: export * as namespace ---

    #[test]
    fn exports_namespace_star_reexport() {
        let result = parse("export * as utils from './utils';");
        assert!(result
            .metadata
            .export_names()
            .contains(&"utils".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils".to_string()));
    }

    // --- ALP-756: export namespace / module ---

    #[test]
    fn exports_namespace_declaration() {
        let result = parse("export namespace Validation { export function isEmail(s: string): boolean { return true; } }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Validation".to_string()));
    }

    #[test]
    fn exports_module_declaration() {
        let result = parse("export module Shapes { export class Circle {} }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Shapes".to_string()));
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

    // --- ALP-749: Barrel re-export dependency capture ---

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
            vec!["UserService", "AuthService", "Logger"]
        );
        // ALP-749: re-export sources must appear in dependencies
        assert!(result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./logger".to_string()));
    }

    #[test]
    fn barrel_reexport_mixed_import_and_export_from() {
        let source = r#"
import { Pool } from './db/pool';
export { UserService } from './user.service';
export { AuthService } from './auth.service';
"#;
        let result = parse(source);
        assert!(result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string()));
    }

    #[test]
    fn reexport_external_package_not_in_dependencies() {
        let result = parse("export { foo } from '@scope/pkg';");
        assert!(!result
            .metadata
            .dependencies
            .contains(&"@scope/pkg".to_string()));
    }

    // --- ALP-750: export * from star re-exports ---

    #[test]
    fn star_reexport_adds_dependency_not_export_name() {
        let result = parse("export * from './utils';");
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils".to_string()));
        assert!(!result.metadata.export_names().contains(&"*".to_string()));
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn star_reexport_external_not_in_dependencies() {
        let result = parse("export * from 'some-package';");
        assert!(result.metadata.dependencies.is_empty());
    }

    // --- ALP-753: TSX parser ---

    #[test]
    fn tsx_jsx_parsed_with_tsx_grammar() {
        let source = r#"
export function Button({ label }: { label: string }) {
    return <button>{label}</button>;
}
"#;
        let result = parse_tsx(source);
        assert!(result
            .metadata
            .export_names()
            .contains(&"Button".to_string()));
    }

    #[test]
    fn tsx_jsx_arrow_component() {
        let source = r#"
export const Card = ({ title }: { title: string }) => (
    <div className="card">
        <h2>{title}</h2>
    </div>
);
"#;
        let result = parse_tsx(source);
        assert!(result.metadata.export_names().contains(&"Card".to_string()));
    }

    #[test]
    fn ts_parser_language_id_and_extensions() {
        let parser = TypeScriptParser::new().unwrap();
        assert_eq!(parser.language_id(), "typescript");
        assert_eq!(parser.extensions(), &["ts", "js"]);
    }

    #[test]
    fn tsx_parser_language_id_and_extensions() {
        let parser = TypeScriptParser::new_tsx().unwrap();
        assert_eq!(parser.language_id(), "tsx");
        assert_eq!(parser.extensions(), &["tsx", "jsx"]);
    }

    // --- ALP-754: Decorator extraction ---

    #[test]
    fn decorator_simple_captured() {
        let source = r#"
@Component
export class AppComponent {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Component"));
    }

    #[test]
    fn decorator_call_expression_captured() {
        let source = r#"
@Injectable()
export class UserService {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Injectable"));
    }

    #[test]
    fn decorator_multiple_unique() {
        let source = r#"
@Controller('/users')
export class UserController {}

@Injectable()
export class AuthService {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Controller"));
        assert!(decorators.contains(&"Injectable"));
    }

    #[test]
    fn no_decorators_custom_fields_none() {
        let result = parse("export class Plain {}");
        assert!(result.custom_fields.is_none());
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
                "DatabaseConfig",
                "DatabaseService",
                "createService",
                "DEFAULT_PORT"
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

    // --- Default export extraction ---

    #[test]
    fn exports_default_function() {
        let result = parse("export default function App() { return null; }");
        assert_eq!(result.metadata.export_names(), vec!["App"]);
    }

    #[test]
    fn exports_default_class() {
        let result = parse("export default class Router { navigate() {} }");
        assert_eq!(result.metadata.export_names(), vec!["Router"]);
    }

    #[test]
    fn exports_default_identifier() {
        let source = "const Component = () => null;\nexport default Component;";
        let result = parse(source);
        assert_eq!(result.metadata.export_names(), vec!["Component"]);
    }

    #[test]
    fn exports_default_anonymous_arrow_skipped() {
        let result = parse("export default () => {};");
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn exports_default_anonymous_object_skipped() {
        let result = parse("export default { key: 'value' };");
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn exports_default_function_line_range() {
        let source = "// header\nexport default function App() {\n  return null;\n}\n";
        let result = parse(source);
        let app = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "App")
            .unwrap();
        assert_eq!(app.start_line, 2);
        assert_eq!(app.end_line, 4);
    }

    #[test]
    fn exports_default_class_line_range() {
        let source = "// header\nexport default class Router {\n  navigate() {}\n}\n";
        let result = parse(source);
        let router = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Router")
            .unwrap();
        assert_eq!(router.start_line, 2);
        assert_eq!(router.end_line, 4);
    }

    #[test]
    fn exports_default_identifier_line_range() {
        let source = "const Foo = 1;\nexport default Foo;\n";
        let result = parse(source);
        let foo = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Foo")
            .unwrap();
        assert_eq!(foo.start_line, 2);
        assert_eq!(foo.end_line, 2);
    }

    // --- Type alias export extraction ---

    #[test]
    fn exports_type_alias() {
        let result = parse("export type User = { name: string; email: string };");
        assert_eq!(result.metadata.export_names(), vec!["User"]);
    }

    #[test]
    fn exports_type_alias_with_generics() {
        let result = parse("export type Nullable<T> = T | null;");
        assert_eq!(result.metadata.export_names(), vec!["Nullable"]);
    }

    #[test]
    fn exports_type_alias_line_range() {
        let source = "// types\nexport type Config = {\n  debug: boolean;\n  port: number;\n};\n";
        let result = parse(source);
        let cfg = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Config")
            .unwrap();
        assert_eq!(cfg.start_line, 2);
        assert_eq!(cfg.end_line, 5);
    }

    // --- Mixed default + named + type exports ---

    #[test]
    fn exports_default_with_named_and_types() {
        let source = r#"
export type Props = { label: string };
export const VERSION = "1.0";
export default function App() { return null; }
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["Props", "VERSION", "App"]
        );
    }

    #[test]
    fn exports_default_identifier_with_named() {
        let source = r#"
export const helper = () => {};
const Main = () => {};
export default Main;
"#;
        let result = parse(source);
        assert_eq!(result.metadata.export_names(), vec!["helper", "Main"]);
    }
}
