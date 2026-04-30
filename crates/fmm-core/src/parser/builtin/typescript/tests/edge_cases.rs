use super::support::parse;
use crate::identity::EdgeKind;

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

#[test]
fn dependency_kind_marks_import_type_as_type_only() {
    let result = parse("import type { Foo } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::TypeOnly)
    );
}

#[test]
fn dependency_kind_marks_value_import_as_runtime() {
    let result = parse("import { Foo } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_marks_mixed_type_and_value_import_as_runtime() {
    let result = parse("import { type Foo, bar } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_default_import_is_runtime() {
    let result = parse("import Foo from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_namespace_import_is_runtime() {
    let result = parse("import * as Foo from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_side_effect_import_is_runtime() {
    let result = parse("import './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_default_identifier_starting_with_type_is_runtime() {
    // Regression: a string-prefix `starts_with("import type")` check would
    // misclassify any default import whose identifier begins with `type`.
    let result = parse("import typescriptCompiler from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_default_with_type_only_named_is_runtime() {
    // Regression: a default value import combined with type-only named
    // imports must collapse to runtime because the default binding itself
    // is a value.
    let result = parse("import Foo, { type Bar } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_value_named_type_is_runtime() {
    // Regression: importing a value identifier called `type` is a runtime
    // edge, not a type-only edge.
    let result = parse("import { type } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_export_type_reexport_is_type_only() {
    let result = parse("export type { Foo } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::TypeOnly)
    );
}

#[test]
fn dependency_kind_export_type_specifier_reexport_is_type_only() {
    let result = parse("export { type Foo } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::TypeOnly)
    );
}

#[test]
fn dependency_kind_value_reexport_is_runtime() {
    let result = parse("export { Foo } from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}

#[test]
fn dependency_kind_wildcard_reexport_is_runtime() {
    let result = parse("export * from './foo';");

    assert_eq!(result.metadata.dependencies, vec!["./foo"]);
    assert_eq!(
        result.metadata.dependency_kinds.get("./foo"),
        Some(&EdgeKind::Runtime)
    );
}
