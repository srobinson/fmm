use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::php::PhpParser;

#[test]
fn empty_file() {
    assert_empty_parse(PhpParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        PhpParser::new().unwrap(),
        "<?php\nclass {{{{ invalid syntax !@#$",
    );
}

#[test]
fn private_and_protected_methods_are_not_exports() {
    let source =
        "<?php\nclass Foo {\n    private function bar() {}\n    protected function baz() {}\n}\n";

    let result = parse_with(PhpParser::new().unwrap(), source);

    assert_exports_include(&result, &["Foo"]);
    assert_exports_exclude(&result, &["bar", "baz"]);
}

#[test]
fn comments_only() {
    assert_no_exports(
        PhpParser::new().unwrap(),
        "<?php\n// Just a comment\n/* Block comment */\n/** Docblock */\n",
    );
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        PhpParser::new().unwrap(),
        "<?php\r\nclass Foo {}\r\nfunction bar() {}\r\n",
    );

    assert_exports_include(&result, &["Foo", "bar"]);
}

#[test]
fn enum_with_methods() {
    let source = "<?php\nenum Color {\n    case Red;\n    case Blue;\n    public function label(): string { return $this->name; }\n}\n";

    let result = parse_with(PhpParser::new().unwrap(), source);

    assert_exports_include(&result, &["Color", "label"]);
}
