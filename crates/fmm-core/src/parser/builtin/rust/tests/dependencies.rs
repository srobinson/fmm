use super::super::rust_use_path_to_dep;
use super::support::parse;

#[test]
fn parse_rust_use_imports() {
    let source = "use std::collections::HashMap;\nuse anyhow::Result;\nuse crate::config::Config;";
    let result = parse(source);
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(!result.metadata.imports.contains(&"crate".to_string()));
}

#[test]
fn parse_rust_extern_crate() {
    let source = "extern crate serde;\nextern crate log;\nuse serde::Deserialize;";
    let result = parse(source);
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));
}

#[test]
fn parse_rust_includes_std_core_alloc() {
    let source = "use std::io;\nuse core::fmt;\nuse alloc::vec::Vec;\nuse tokio::runtime;";
    let result = parse(source);
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"core".to_string()));
    assert!(result.metadata.imports.contains(&"alloc".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));
}

#[test]
fn parse_rust_crate_deps() {
    let source = "use crate::config::Config;\nuse super::utils;";
    let result = parse(source);
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"crate::config".to_string()),
        "expected crate::config in {:?}",
        deps
    );
    assert!(
        deps.contains(&"../utils".to_string()),
        "expected ../utils in {:?}",
        deps
    );
    assert!(!deps.contains(&"std".to_string()));
}

#[test]
fn rust_use_path_to_dep_conversions() {
    assert_eq!(
        rust_use_path_to_dep("crate::config::Config"),
        Some("crate::config".into())
    );
    assert_eq!(
        rust_use_path_to_dep("crate::parser::builtin::rust"),
        Some("crate::parser::builtin::rust".into())
    );
    assert_eq!(
        rust_use_path_to_dep("super::utils"),
        Some("../utils".into())
    );
    assert_eq!(
        rust_use_path_to_dep("super::parser::builtin"),
        Some("../parser/builtin".into())
    );
    assert_eq!(rust_use_path_to_dep("std::collections::HashMap"), None);
    assert_eq!(rust_use_path_to_dep("anyhow"), None);
}

#[test]
fn wildcard_use_crate_module_recorded_as_dep() {
    let source = "use crate::parser::*;";
    let result = parse(source);
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"crate::parser".to_string()),
        "expected crate::parser in deps {:?}",
        deps
    );
}

#[test]
fn wildcard_use_super_module_recorded_as_dep() {
    let source = "use super::utils::*;";
    let result = parse(source);
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"../utils".to_string()),
        "expected ../utils in deps {:?}",
        deps
    );
}

#[test]
fn wildcard_use_external_crate_not_a_dep() {
    let source = "use std::io::*;";
    let result = parse(source);
    let deps = &result.metadata.dependencies;
    assert!(
        deps.is_empty(),
        "std wildcard should produce no local dep, got {:?}",
        deps
    );
    assert!(
        result.metadata.imports.contains(&"std".to_string()),
        "std should be in imports"
    );
}
