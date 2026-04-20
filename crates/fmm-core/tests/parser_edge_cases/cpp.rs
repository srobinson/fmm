use super::support::{
    assert_empty_parse, assert_exports_include, assert_imports_include, assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::cpp::CppParser;

#[test]
fn empty_file() {
    assert_empty_parse(CppParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(CppParser::new().unwrap(), "class {{{{ not valid }");
}

#[test]
fn comments_only() {
    let result = parse_with(
        CppParser::new().unwrap(),
        "// Comment\n/* Block */\n/// Doc\n",
    );

    assert!(result.metadata.exports.is_empty());
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        CppParser::new().unwrap(),
        "#include <vector>\r\nclass Foo {};\r\n",
    );

    assert_imports_include(&result, &["vector"]);
    assert_exports_include(&result, &["Foo"]);
}
