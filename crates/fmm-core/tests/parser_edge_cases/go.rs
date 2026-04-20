use super::support::{assert_empty_parse, assert_no_exports, assert_parse_ok, parse_with};
use fmm_core::parser::builtin::go::GoParser;

#[test]
fn empty_file() {
    assert_empty_parse(GoParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        GoParser::new().unwrap(),
        "package main\n\nfunc {{{{ invalid",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        GoParser::new().unwrap(),
        "package main\n\nfunc private() {}\nvar internal = 1\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        GoParser::new().unwrap(),
        "// Just a comment\n/* Block comment */\n",
    );

    assert!(result.metadata.exports.is_empty());
}
