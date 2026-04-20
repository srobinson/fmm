use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_imports_include,
    assert_no_exports, assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::scala::ScalaParser;

#[test]
fn empty_file() {
    assert_empty_parse(ScalaParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        ScalaParser::new().unwrap(),
        "class {{{{ invalid syntax !@#$",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        ScalaParser::new().unwrap(),
        "private class Foo\nprivate object Bar\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        ScalaParser::new().unwrap(),
        "// Line comment\n/* Block comment */\n/** Scaladoc */\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        ScalaParser::new().unwrap(),
        "import scala.io\r\nclass Foo\r\nobject Bar\r\n",
    );

    assert_imports_include(&result, &["scala"]);
    assert_exports_include(&result, &["Foo", "Bar"]);
}

#[test]
fn protected_excluded() {
    let result = parse_with(
        ScalaParser::new().unwrap(),
        "protected class Foo\nclass Bar\n",
    );

    assert_exports_include(&result, &["Bar"]);
    assert_exports_exclude(&result, &["Foo"]);
}
