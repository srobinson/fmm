use super::support::{
    assert_empty_parse, assert_exports_include, assert_imports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::zig::ZigParser;

#[test]
fn empty_file() {
    assert_empty_parse(ZigParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(ZigParser::new().unwrap(), "pub fn {{{{ invalid syntax !@#$");
}

#[test]
fn no_exports() {
    assert_no_exports(
        ZigParser::new().unwrap(),
        "const internal: u32 = 42;\nfn private() void {}\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        ZigParser::new().unwrap(),
        "// Just a comment\n/// Doc comment\n//! Module doc\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        ZigParser::new().unwrap(),
        "const std = @import(\"std\");\r\npub fn hello() void {}\r\n",
    );

    assert_imports_include(&result, &["std"]);
    assert_exports_include(&result, &["hello"]);
}

#[test]
fn error_set() {
    let result = parse_with(
        ZigParser::new().unwrap(),
        "pub const MyError = error{ Foo, Bar, Baz };\n",
    );

    assert_exports_include(&result, &["MyError"]);
}
