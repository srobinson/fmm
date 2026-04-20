use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::swift::SwiftParser;

#[test]
fn empty_file() {
    assert_empty_parse(SwiftParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        SwiftParser::new().unwrap(),
        "public func {{{{ invalid syntax !@#$",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        SwiftParser::new().unwrap(),
        "private func helper() {}\ninternal struct Config {}\nfunc defaultFunc() {}\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        SwiftParser::new().unwrap(),
        "// Line comment\n/* Block comment */\n/// Doc comment\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        SwiftParser::new().unwrap(),
        "import Foundation\r\npublic func hello() {}\r\npublic struct Point {}\r\n",
    );

    assert_exports_include(&result, &["hello", "Point"]);
}

#[test]
fn mixed_visibility() {
    let source = "public func visible() {}\nprivate func hidden() {}\nfileprivate func alsoHidden() {}\nopen class Base {}\ninternal class NotExported {}\n";

    let result = parse_with(SwiftParser::new().unwrap(), source);

    assert_exports_include(&result, &["visible", "Base"]);
    assert_exports_exclude(&result, &["hidden", "alsoHidden", "NotExported"]);
    assert_eq!(result.metadata.export_names().len(), 2);
}
