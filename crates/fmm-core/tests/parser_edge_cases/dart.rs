use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::dart::DartParser;

#[test]
fn empty_file() {
    assert_empty_parse(DartParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(DartParser::new().unwrap(), "class {{{{ invalid !@#$");
}

#[test]
fn no_exports() {
    assert_no_exports(
        DartParser::new().unwrap(),
        "void _helper() {}\nclass _Internal {}\n",
    );
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        DartParser::new().unwrap(),
        "import 'dart:io';\r\nvoid hello() {}\r\nclass MyClass {}\r\n",
    );

    assert_exports_include(&result, &["hello", "MyClass"]);
}

#[test]
fn underscore_privacy() {
    let source = "class Public {}\nclass _Private {}\nvoid visible() {}\nvoid _hidden() {}\ntypedef Pub = void Function();\ntypedef _Priv = void Function();\n";

    let result = parse_with(DartParser::new().unwrap(), source);

    assert_exports_include(&result, &["Public", "visible", "Pub"]);
    assert_exports_exclude(&result, &["_Private", "_hidden", "_Priv"]);
    assert_eq!(result.metadata.export_names().len(), 3);
}

#[test]
fn whitespace_only() {
    assert_no_exports(DartParser::new().unwrap(), "   \n\n  \n");
}
