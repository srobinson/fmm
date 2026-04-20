use fmm_core::parser::{ParseResult, Parser};

pub fn parse_with<P: Parser>(mut parser: P, source: &str) -> ParseResult {
    parser.parse(source).unwrap()
}

pub fn assert_empty_parse<P: Parser>(parser: P) {
    let result = parse_with(parser, "");
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

pub fn assert_parse_ok<P: Parser>(mut parser: P, source: &str) {
    assert!(parser.parse(source).is_ok());
}

pub fn assert_no_exports<P: Parser>(parser: P, source: &str) {
    let result = parse_with(parser, source);
    assert!(result.metadata.exports.is_empty());
}

pub fn assert_exports_include(result: &ParseResult, expected: &[&str]) {
    let names = result.metadata.export_names();
    for expected_name in expected {
        assert!(
            names.iter().any(|actual| actual == expected_name),
            "missing export {expected_name}; exports: {names:?}"
        );
    }
}

pub fn assert_exports_exclude(result: &ParseResult, absent: &[&str]) {
    let names = result.metadata.export_names();
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
