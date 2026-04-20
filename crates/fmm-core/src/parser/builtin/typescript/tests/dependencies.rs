use super::support::parse;

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
