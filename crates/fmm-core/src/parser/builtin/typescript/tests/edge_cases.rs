use super::support::parse;

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
