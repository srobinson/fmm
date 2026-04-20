use super::support::{
    assert_empty_parse, assert_exports_include, assert_no_exports, assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::typescript::TypeScriptParser;

#[test]
fn empty_file() {
    assert_empty_parse(TypeScriptParser::new().unwrap());
}

#[test]
fn huge_file() {
    let mut lines = vec!["import { Foo } from 'bar';".to_string()];
    for i in 0..10_000 {
        lines.push(format!("export const x{} = {};", i, i));
    }

    let result = parse_with(TypeScriptParser::new().unwrap(), &lines.join("\n"));

    assert!(result.metadata.exports.len() > 1000);
    assert_eq!(result.metadata.loc, 10_001);
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        TypeScriptParser::new().unwrap(),
        "export function {{{{ invalid syntax !@#$",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        TypeScriptParser::new().unwrap(),
        "const internal = 42;\nfunction helper() { return internal; }",
    );
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        TypeScriptParser::new().unwrap(),
        "export function hello() {}\r\nexport const world = 42;\r\n",
    );

    assert_exports_include(&result, &["hello", "world"]);
}

#[test]
fn whitespace_only() {
    assert_no_exports(TypeScriptParser::new().unwrap(), "   \n\n  \t  \n");
}

#[test]
fn comments_only() {
    let result = parse_with(
        TypeScriptParser::new().unwrap(),
        "// Just a comment\n/* Block comment */\n// Another comment\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn default_anonymous_arrow_not_captured() {
    assert_no_exports(TypeScriptParser::new().unwrap(), "export default () => {};");
}

#[test]
fn default_anonymous_class_not_captured() {
    assert_no_exports(
        TypeScriptParser::new().unwrap(),
        "export default class { run() {} }",
    );
}

#[test]
fn default_object_literal_not_captured() {
    assert_no_exports(
        TypeScriptParser::new().unwrap(),
        "export default { key: 'value', num: 42 };",
    );
}

#[test]
fn default_and_named_coexist() {
    let source = r#"
export const helper = () => {};
export function util() {}
export default function Main() {}
"#;

    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert_eq!(
        result.metadata.export_names(),
        vec!["helper", "util", "Main"]
    );
}

#[test]
fn type_only_exports() {
    let source = r#"
export type A = string;
export type B = number;
export type C = A | B;
"#;

    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert_eq!(result.metadata.export_names(), vec!["A", "B", "C"]);
}

#[test]
fn default_identifier_deduplicates_with_named() {
    let result = parse_with(
        TypeScriptParser::new().unwrap(),
        "export { Component };\nexport default Component;",
    );

    assert_eq!(result.metadata.export_names(), vec!["Component"]);
}
