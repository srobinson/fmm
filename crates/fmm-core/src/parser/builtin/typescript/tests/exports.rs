use super::support::parse;

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
