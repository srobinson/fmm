use super::support::{assert_empty_parse, assert_parse_ok, parse_with};
use fmm_core::parser::builtin::java::JavaParser;

#[test]
fn empty_file() {
    assert_empty_parse(JavaParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(JavaParser::new().unwrap(), "public class {{{{ invalid }");
}

#[test]
fn comments_only() {
    let result = parse_with(
        JavaParser::new().unwrap(),
        "// Comment\n/** Javadoc */\n/* Block */\n",
    );

    assert!(result.metadata.exports.is_empty());
}
