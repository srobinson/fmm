use fmm_core::parser::{ParseResult, Parser};
use serde_json::Value;

pub fn parse_with<P: Parser>(mut parser: P, source: &str) -> ParseResult {
    parser.parse(source).unwrap()
}

pub fn export_names(result: &ParseResult) -> Vec<String> {
    result.metadata.export_names()
}

pub fn assert_exports_include(result: &ParseResult, expected: &[&str]) {
    let names = export_names(result);
    for expected_name in expected {
        assert!(
            names.iter().any(|actual| actual == expected_name),
            "missing export {expected_name}; exports: {names:?}"
        );
    }
}

pub fn assert_exports_exclude(result: &ParseResult, absent: &[&str]) {
    let names = export_names(result);
    for absent_name in absent {
        assert!(
            !names.iter().any(|actual| actual == absent_name),
            "unexpected export {absent_name}; exports: {names:?}"
        );
    }
}

pub fn assert_imports_include(result: &ParseResult, expected: &[&str]) {
    for expected_import in expected {
        assert!(
            result
                .metadata
                .imports
                .iter()
                .any(|actual| actual == expected_import),
            "missing import {expected_import}; imports: {:?}",
            result.metadata.imports
        );
    }
}

pub fn assert_dependencies_include(result: &ParseResult, expected: &[&str]) {
    for expected_dep in expected {
        assert!(
            result
                .metadata
                .dependencies
                .iter()
                .any(|actual| actual == expected_dep),
            "missing dependency {expected_dep}; dependencies: {:?}",
            result.metadata.dependencies
        );
    }
}

pub fn custom_fields(result: &ParseResult) -> &std::collections::HashMap<String, Value> {
    result
        .custom_fields
        .as_ref()
        .expect("should have custom fields")
}

pub fn custom_string_array(result: &ParseResult, key: &str) -> Vec<String> {
    custom_fields(result)
        .get(key)
        .unwrap_or_else(|| panic!("missing custom field {key}"))
        .as_array()
        .unwrap_or_else(|| panic!("custom field {key} should be an array"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("custom field {key} should contain strings"))
                .to_string()
        })
        .collect()
}

pub fn custom_u64(result: &ParseResult, key: &str) -> u64 {
    custom_fields(result)
        .get(key)
        .unwrap_or_else(|| panic!("missing custom field {key}"))
        .as_u64()
        .unwrap_or_else(|| panic!("custom field {key} should be an integer"))
}
