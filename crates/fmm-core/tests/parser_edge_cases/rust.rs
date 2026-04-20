use super::support::{
    assert_empty_parse, assert_exports_include, assert_no_exports, assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::rust::RustParser;

#[test]
fn empty_file() {
    assert_empty_parse(RustParser::new().unwrap());
}

#[test]
fn huge_file() {
    let mut lines = vec!["use std::io;".to_string()];
    for i in 0..10_000 {
        lines.push(format!("pub fn func_{}() {{}}", i));
    }

    let result = parse_with(RustParser::new().unwrap(), &lines.join("\n"));

    assert!(result.metadata.exports.len() > 1000);
    assert_eq!(result.metadata.loc, 10_001);
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        RustParser::new().unwrap(),
        "pub fn {{{{ not valid rust }}}}",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        RustParser::new().unwrap(),
        "fn private_fn() {}\nstruct Internal {}",
    );
}

#[test]
fn unicode_in_strings() {
    let result = parse_with(
        RustParser::new().unwrap(),
        "pub fn hello() -> &'static str { \"こんにちは\" }",
    );

    assert_exports_include(&result, &["hello"]);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        RustParser::new().unwrap(),
        "pub fn hello() {}\r\npub struct World {}\r\n",
    );

    assert_exports_include(&result, &["hello", "World"]);
}

#[test]
fn whitespace_only() {
    assert_no_exports(RustParser::new().unwrap(), "   \n\n  \t  \n");
}

#[test]
fn comments_only() {
    let result = parse_with(
        RustParser::new().unwrap(),
        "// Just a comment\n/// Doc comment\n//! Module doc\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}
