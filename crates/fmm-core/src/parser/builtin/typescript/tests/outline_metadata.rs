use super::support::parse;
use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};

#[test]
fn typescript_exports_carry_outline_metadata() {
    let source = r#"
export function run(input: string): string {
  return input;
}

export const LIMIT = 3;
export class Service {}
export interface Config { debug: boolean; }
export type Name = string;
export enum Mode { Fast }
export { run as execute };
export default Service;
export namespace Validation { export const ready = true; }
export module Shapes {}
export * as utils from './utils';
"#;

    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_entry(
        exports,
        "run",
        SymbolVisibility::Public,
        DeclarationKind::Fn,
    );
    assert_entry(
        exports,
        "LIMIT",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "Service",
        SymbolVisibility::Public,
        DeclarationKind::Struct,
    );
    assert_entry(
        exports,
        "Config",
        SymbolVisibility::Public,
        DeclarationKind::Trait,
    );
    assert_entry(
        exports,
        "Name",
        SymbolVisibility::Public,
        DeclarationKind::Type,
    );
    assert_entry(
        exports,
        "Mode",
        SymbolVisibility::Public,
        DeclarationKind::Enum,
    );
    assert_entry(
        exports,
        "execute",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "Validation",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "Shapes",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "utils",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
}

#[test]
fn typescript_class_members_carry_outline_metadata() {
    let source = r#"
export class Service {
  public ready = true;
  protected cache: Map<string, string>;
  private token = "secret";
  #session = createSession();

  run(input: string): void {}
  protected reset(): void {}
  private destroy(): void {}
}
"#;

    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_child_entry(
        exports,
        "Service",
        "ready",
        SymbolVisibility::Public,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Service",
        "cache",
        SymbolVisibility::Protected,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Service",
        "token",
        SymbolVisibility::Private,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Service",
        "#session",
        SymbolVisibility::Private,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Service",
        "run",
        SymbolVisibility::Public,
        DeclarationKind::Method,
    );
    assert_child_entry(
        exports,
        "Service",
        "reset",
        SymbolVisibility::Protected,
        DeclarationKind::Method,
    );
    assert_child_entry(
        exports,
        "Service",
        "destroy",
        SymbolVisibility::Private,
        DeclarationKind::Method,
    );
}

#[test]
fn typescript_nested_and_test_symbols_carry_outline_metadata() {
    let source = r#"
export function outer(): void {
  const state: State = createState();
  function inner(): void {}
}

describe("suite", () => {
  it("runs", () => {});
  test("also runs", () => {});
});
"#;

    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_child_entry(
        exports,
        "outer",
        "state",
        SymbolVisibility::NonExported,
        DeclarationKind::Const,
    );
    assert_child_entry(
        exports,
        "outer",
        "inner",
        SymbolVisibility::NonExported,
        DeclarationKind::Fn,
    );
    assert_entry(
        exports,
        "describe suite",
        SymbolVisibility::NonExported,
        DeclarationKind::Test,
    );
    assert_entry(
        exports,
        "it runs",
        SymbolVisibility::NonExported,
        DeclarationKind::Test,
    );
    assert_entry(
        exports,
        "test also runs",
        SymbolVisibility::NonExported,
        DeclarationKind::Test,
    );
}

#[test]
fn typescript_signature_is_declaration_header_text() {
    let source = r#"
export function run(input: string): string {
  return input;
}

export class Service {
  private token: string;

  run(input: string): void {}
}
"#;

    let result = parse(source);
    let run = find_entry(&result.metadata.exports, "run");
    assert_eq!(
        run.signature.as_deref(),
        Some("export function run(input: string): string")
    );

    let class = find_entry(&result.metadata.exports, "Service");
    assert_eq!(class.signature.as_deref(), Some("export class Service"));

    let token = find_child_entry(&result.metadata.exports, "Service", "token");
    assert_eq!(token.signature.as_deref(), Some("private token: string"));

    let method = find_child_entry(&result.metadata.exports, "Service", "run");
    assert_eq!(
        method.signature.as_deref(),
        Some("run(input: string): void")
    );
}

fn assert_entry(
    exports: &[ExportEntry],
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) {
    let entry = find_entry(exports, name);
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(entry.signature.is_some(), "{name} should carry a signature");
}

fn assert_child_entry(
    exports: &[ExportEntry],
    parent: &str,
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) {
    let entry = find_child_entry(exports, parent, name);
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(
        entry.signature.is_some(),
        "{parent}.{name} should carry a signature"
    );
}

fn find_entry<'a>(exports: &'a [ExportEntry], name: &str) -> &'a ExportEntry {
    exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.is_none())
        .unwrap_or_else(|| panic!("{name} should be indexed"))
}

fn find_child_entry<'a>(exports: &'a [ExportEntry], parent: &str, name: &str) -> &'a ExportEntry {
    exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.as_deref() == Some(parent))
        .unwrap_or_else(|| panic!("{parent}.{name} should be indexed"))
}
