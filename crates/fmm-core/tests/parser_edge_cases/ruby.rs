use super::support::{assert_empty_parse, assert_no_exports, assert_parse_ok, parse_with};
use fmm_core::parser::builtin::ruby::RubyParser;

#[test]
fn empty_file() {
    assert_empty_parse(RubyParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(RubyParser::new().unwrap(), "class !!!! end");
}

#[test]
fn comments_only() {
    let result = parse_with(
        RubyParser::new().unwrap(),
        "# Just a comment\n# Another comment\n",
    );

    assert!(result.metadata.exports.is_empty());
}

#[test]
fn no_exports_all_private() {
    assert_no_exports(RubyParser::new().unwrap(), "def _private\n  nil\nend\n");
}
