use super::support::{
    assert_empty_parse, assert_exports_include, assert_imports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::c::CParser;

#[test]
fn empty_file() {
    assert_empty_parse(CParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(CParser::new().unwrap(), "int {{{{ invalid syntax !@#$");
}

#[test]
fn no_exports() {
    assert_no_exports(
        CParser::new().unwrap(),
        "static int helper() { return 0; }\nstatic void internal() {}\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        CParser::new().unwrap(),
        "/* Block comment */\n// Line comment\n/* Another block */\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        CParser::new().unwrap(),
        "#include <stdio.h>\r\nint main() { return 0; }\r\n",
    );

    assert_imports_include(&result, &["stdio.h"]);
    assert_exports_include(&result, &["main"]);
}

#[test]
fn header_only_macros() {
    let source =
        "#ifndef GUARD_H\n#define GUARD_H\n#define MAX 100\n#define VERSION \"1.0\"\n#endif\n";

    let result = parse_with(CParser::new().unwrap(), source);

    assert_exports_include(&result, &["GUARD_H", "MAX", "VERSION"]);
}
