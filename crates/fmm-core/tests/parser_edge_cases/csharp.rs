use super::support::{assert_empty_parse, assert_parse_ok, parse_with};
use fmm_core::parser::builtin::csharp::CSharpParser;

#[test]
fn empty_file() {
    assert_empty_parse(CSharpParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        CSharpParser::new().unwrap(),
        "public class {{{{ not valid }",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        CSharpParser::new().unwrap(),
        "// Comment\n/// Doc comment\n/* Block */\n",
    );

    assert!(result.metadata.exports.is_empty());
}
