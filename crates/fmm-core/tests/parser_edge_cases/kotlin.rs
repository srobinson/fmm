use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::kotlin::KotlinParser;

#[test]
fn empty_file() {
    assert_empty_parse(KotlinParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(KotlinParser::new().unwrap(), "fun {{{{ invalid syntax !@#$");
}

#[test]
fn no_exports() {
    assert_no_exports(
        KotlinParser::new().unwrap(),
        "private fun helper() {}\ninternal class Config\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        KotlinParser::new().unwrap(),
        "// Line comment\n/* Block comment */\n/** KDoc comment */\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        KotlinParser::new().unwrap(),
        "import kotlin.collections.List\r\nfun hello() {}\r\nclass MyClass\r\n",
    );

    assert_exports_include(&result, &["hello", "MyClass"]);
}

#[test]
fn default_public_vs_private() {
    let source = "fun visible() {}\nprivate fun hidden() {}\ninternal fun alsoHidden() {}\nclass MyClass\nprivate class Secret\n";

    let result = parse_with(KotlinParser::new().unwrap(), source);

    assert_exports_include(&result, &["visible", "MyClass"]);
    assert_exports_exclude(&result, &["hidden", "alsoHidden", "Secret"]);
    assert_eq!(result.metadata.export_names().len(), 2);
}
