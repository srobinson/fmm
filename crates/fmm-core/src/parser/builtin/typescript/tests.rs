use std::collections::HashMap;

use super::TypeScriptParser;
use super::tsconfig::{match_alias, strip_json_comments};
use crate::parser::{ParseResult, Parser};

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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"greet".to_string())
    );
}

#[test]
fn exports_arrow_function_via_const() {
    let result = parse("export const add = (a: number, b: number) => a + b;");
    assert!(result.metadata.export_names().contains(&"add".to_string()));
}

#[test]
fn exports_class() {
    let result = parse("export class UserService { constructor() {} }");
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"UserService".to_string())
    );
}

#[test]
fn exports_interface() {
    let result = parse("export interface Config { debug: boolean; }");
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Config".to_string())
    );
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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"MAX_RETRIES".to_string())
    );
}

#[test]
fn exports_let_variable() {
    let result = parse("export let counter = 0;");
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"counter".to_string())
    );
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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Direction".to_string())
    );
}

#[test]
fn exports_const_enum() {
    let result = parse("export const enum Status { Active, Inactive }");
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Status".to_string())
    );
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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"utils".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./utils".to_string())
    );
}

// --- ALP-756: export namespace / module ---

#[test]
fn exports_namespace_declaration() {
    let result = parse(
        "export namespace Validation { export function isEmail(s: string): boolean { return true; } }",
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Validation".to_string())
    );
}

#[test]
fn exports_module_declaration() {
    let result = parse("export module Shapes { export class Circle {} }");
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Shapes".to_string())
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
    assert!(
        result
            .metadata
            .imports
            .contains(&"@types/express".to_string())
    );
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
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../lib/bar".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"/absolute/baz".to_string())
    );
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
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./logger".to_string())
    );
}

#[test]
fn barrel_reexport_mixed_import_and_export_from() {
    let source = r#"
import { Pool } from './db/pool';
export { UserService } from './user.service';
export { AuthService } from './auth.service';
"#;
    let result = parse(source);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string())
    );
}

#[test]
fn reexport_external_package_not_in_dependencies() {
    let result = parse("export { foo } from '@scope/pkg';");
    assert!(
        !result
            .metadata
            .dependencies
            .contains(&"@scope/pkg".to_string())
    );
}

// --- ALP-750: export * from star re-exports ---

#[test]
fn star_reexport_adds_dependency_not_export_name() {
    let result = parse("export * from './utils';");
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./utils".to_string())
    );
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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Button".to_string())
    );
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
    assert_eq!(Parser::language_id(&parser), "typescript");
    assert_eq!(Parser::extensions(&parser), &["ts", "js"]);
}

#[test]
fn tsx_parser_language_id_and_extensions() {
    let parser = TypeScriptParser::new_tsx().unwrap();
    assert_eq!(Parser::language_id(&parser), "tsx");
    assert_eq!(Parser::extensions(&parser), &["tsx", "jsx"]);
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
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./config".to_string())
    );
    assert!(result.metadata.loc > 20);
}

// --- ALP-768: Public class method extraction ---

#[test]
fn class_public_method_indexed() {
    let source = "export class Foo {\n  public bar(): void {}\n}\n";
    let result = parse(source);
    let method = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "bar")
        .unwrap();
    assert_eq!(method.parent_class.as_deref(), Some("Foo"));
    assert_eq!(method.start_line, 2);
    // export_names() excludes methods
    assert!(!result.metadata.export_names().contains(&"bar".to_string()));
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
}

#[test]
fn class_private_method_not_indexed() {
    let source = "export class Foo {\n  private baz(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "baz"));
}

#[test]
fn class_protected_method_not_indexed() {
    let source = "export class Foo {\n  protected qux(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "qux"));
}

#[test]
fn class_constructor_indexed() {
    let source = "export class Foo {\n  constructor(x: number) {}\n}\n";
    let result = parse(source);
    let ctor = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "constructor");
    assert!(ctor.is_some(), "constructor should be indexed");
    assert_eq!(ctor.unwrap().parent_class.as_deref(), Some("Foo"));
}

#[test]
fn class_no_modifier_is_public() {
    let source = "export class Foo {\n  doThing(): void {}\n}\n";
    let result = parse(source);
    assert!(result.metadata.exports.iter().any(|e| e.name == "doThing"));
}

#[test]
fn non_exported_class_methods_not_indexed() {
    let source = "class Internal {\n  run(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "run"));
}

#[test]
fn class_method_line_range_correct() {
    let source = "export class Svc {\n  create() {\n    return 1;\n  }\n  destroy() {}\n}\n";
    let result = parse(source);
    let create = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "create")
        .unwrap();
    assert_eq!(create.start_line, 2);
    assert_eq!(create.end_line, 4);
    let destroy = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "destroy")
        .unwrap();
    assert_eq!(destroy.start_line, 5);
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

// --- ALP-794: tsconfig path alias resolution ---

fn parse_with_aliases_helper(source: &str, aliases: HashMap<String, Vec<String>>) -> ParseResult {
    let mut parser = TypeScriptParser::new().unwrap();
    parser.parse_with_aliases(source, &aliases).unwrap()
}

#[test]
fn alias_wildcard_classified_as_dependency() {
    let mut aliases = HashMap::new();
    aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { helper } from "@/utils/helper";"#;
    let result = parse_with_aliases_helper(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/utils/helper".to_string()),
        "alias import should be a dependency, got: {:?}",
        result.metadata.dependencies
    );
    assert!(
        !result
            .metadata
            .imports
            .contains(&"@/utils/helper".to_string()),
        "alias import must not appear in imports, got: {:?}",
        result.metadata.imports
    );
}

#[test]
fn scoped_package_without_alias_stays_external() {
    let mut aliases = HashMap::new();
    aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { Injectable } from "@nestjs/common";"#;
    let result = parse_with_aliases_helper(source, aliases);
    assert!(
        result
            .metadata
            .imports
            .contains(&"@nestjs/common".to_string()),
        "@nestjs/common must stay in imports, got: {:?}",
        result.metadata.imports
    );
    assert!(
        result.metadata.dependencies.is_empty(),
        "no deps expected, got: {:?}",
        result.metadata.dependencies
    );
}

#[test]
fn no_aliases_falls_back_to_heuristic() {
    // Without tsconfig aliases, @/ imports remain as external (existing behavior).
    let source = r#"import { x } from "@/utils/helper";"#;
    let result = parse(source);
    assert!(
        result
            .metadata
            .imports
            .contains(&"@/utils/helper".to_string()),
        "without aliases, @/ import should stay in imports, got: {:?}",
        result.metadata.imports
    );
}

#[test]
fn alias_tilde_pattern() {
    let mut aliases = HashMap::new();
    aliases.insert("~/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { config } from "~/config/app";"#;
    let result = parse_with_aliases_helper(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/config/app".to_string()),
        "tilde alias should resolve to dependency, got: {:?}",
        result.metadata.dependencies
    );
}

#[test]
fn alias_exact_pattern() {
    let mut aliases = HashMap::new();
    aliases.insert("@app".to_string(), vec!["src/app".to_string()]);
    let source = r#"import App from "@app";"#;
    let result = parse_with_aliases_helper(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/app".to_string()),
        "exact alias should resolve, got: {:?}",
        result.metadata.dependencies
    );
}

// --- ALP-881: named_imports and namespace_imports ---

#[test]
fn named_imports_basic() {
    let result = parse("import { foo, bar } from './mod';");
    let ni = &result.metadata.named_imports;
    assert_eq!(ni.get("./mod").unwrap(), &vec!["bar", "foo"]);
}

#[test]
fn named_imports_aliased_stores_original_name() {
    // `import { foo as bar }` → store `foo`, not `bar`
    let result = parse("import { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(
        names.contains(&"scheduleUpdateOnFiber".to_string()),
        "should store original name"
    );
    assert!(
        !names.contains(&"schedule".to_string()),
        "should not store alias"
    );
}

#[test]
fn named_imports_default_import_not_included() {
    // Default imports do not name a specific export by key
    let result = parse("import React from 'react';");
    assert!(
        result.metadata.named_imports.is_empty(),
        "default imports should not appear in named_imports"
    );
}

#[test]
fn namespace_imports_captured() {
    let result = parse("import * as NS from './module';");
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"./module".to_string())
    );
    assert!(
        result.metadata.named_imports.is_empty(),
        "namespace import should not populate named_imports"
    );
}

#[test]
fn named_reexports_captured() {
    // `export { foo } from './mod'` — captured in named_imports for the source module
    let result = parse("export { scheduleUpdateOnFiber } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
}

#[test]
fn wildcard_reexport_goes_to_namespace_imports() {
    let result = parse("export * from './utils';");
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"./utils".to_string())
    );
}

#[test]
fn type_only_import_included_in_named_imports() {
    let result = parse("import type { Foo } from './types';");
    let ni = &result.metadata.named_imports;
    assert!(
        ni.contains_key("./types"),
        "type-only import should be included"
    );
    assert!(ni["./types"].contains(&"Foo".to_string()));
}

#[test]
fn named_imports_multiple_sources() {
    let source = r#"
import { a, b } from './mod-a';
import { c } from './mod-b';
"#;
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(ni["./mod-a"], vec!["a", "b"]);
    assert_eq!(ni["./mod-b"], vec!["c"]);
}

#[test]
fn named_reexport_aliased_stores_original_name() {
    // `export { foo as bar } from './mod'` → store `foo`
    let result = parse("export { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
    assert!(!names.contains(&"schedule".to_string()));
}

#[test]
fn strip_json_comments_basic() {
    let input = r#"{ // a comment
  "key": "value" // inline comment
}"#;
    let stripped = strip_json_comments(input);
    let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
    assert_eq!(parsed["key"], "value");
}

#[test]
fn match_alias_wildcard() {
    let targets = vec!["src/*".to_string()];
    assert_eq!(
        match_alias("@/utils/helper", "@/*", &targets),
        Some("src/utils/helper".to_string())
    );
    assert_eq!(match_alias("@nestjs/common", "@/*", &targets), None);
}

// --- ALP-922: Nested symbol extraction ---

#[test]
fn nested_fn_extracted_from_exported_function() {
    let source = r#"
export function createTypeChecker(host: any): any {
  var silentNeverType = createIntrinsicType(TypeFlags.Never, "never");
  function getIndexType(type: any): any { return undefined; }
  function getReturnType(sig: any): any { return undefined; }
  return {};
}
"#;
    let result = parse(source);
    let nested: Vec<_> = result
        .metadata
        .exports
        .iter()
        .filter(|e| e.parent_class.as_deref() == Some("createTypeChecker"))
        .collect();
    let names: Vec<&str> = nested.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.contains(&"getIndexType"),
        "getIndexType missing; names={:?}",
        names
    );
    assert!(
        names.contains(&"getReturnType"),
        "getReturnType missing; names={:?}",
        names
    );
    // silentNeverType is closure-state (call expression initializer)
    assert!(
        names.contains(&"silentNeverType"),
        "silentNeverType missing; names={:?}",
        names
    );
}

#[test]
fn nested_fn_has_correct_kind() {
    let source = r#"
export function outer(): void {
  var state = createState();
  function inner(): void {}
}
"#;
    let result = parse(source);
    let inner_entry = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "inner")
        .expect("inner not found");
    assert_eq!(inner_entry.kind.as_deref(), Some("nested-fn"));
    assert_eq!(inner_entry.parent_class.as_deref(), Some("outer"));

    let state_entry = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "state")
        .expect("state (closure-state) not found");
    assert_eq!(state_entry.kind.as_deref(), Some("closure-state"));
}

#[test]
fn trivial_var_not_extracted_as_closure_state() {
    let source = r#"
export function outer(): void {
  let counter = 0;
  var flag = false;
  function inner(): void {}
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    // trivial literals must not appear
    assert!(
        !names.contains(&"counter"),
        "trivial counter should not be extracted"
    );
    assert!(
        !names.contains(&"flag"),
        "trivial flag should not be extracted"
    );
}

#[test]
fn depth2_nested_fn_not_extracted() {
    let source = r#"
export function outer(): void {
  function depth1(): void {
function depth2(): void {}
  }
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&"depth2"), "depth2 should not be extracted");
    assert!(names.contains(&"depth1"), "depth1 should be extracted");
}

#[test]
fn prologue_var_after_first_nested_fn_not_extracted() {
    let source = r#"
export function outer(): void {
  var before = createA();
  function inner(): void {}
  var after = createB();
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        names.contains(&"before"),
        "before (prologue) should be extracted"
    );
    assert!(
        !names.contains(&"after"),
        "after (post-first-fn) should not be extracted"
    );
}

#[test]
fn nested_symbols_in_non_exported_function() {
    let source = r#"
function internalHelper(): void {
  var state = createState();
  function processItem(item: any): void {}
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        names.contains(&"processItem"),
        "processItem should be extracted"
    );
    assert!(
        names.contains(&"state"),
        "state closure-state should be extracted"
    );
}
