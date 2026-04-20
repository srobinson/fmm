use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::lua::LuaParser;

#[test]
fn empty_file() {
    assert_empty_parse(LuaParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        LuaParser::new().unwrap(),
        "function {{{{ invalid syntax !@#$",
    );
}

#[test]
fn no_exports() {
    assert_no_exports(
        LuaParser::new().unwrap(),
        "local function helper() return true end\nlocal x = 42\n",
    );
}

#[test]
fn comments_only() {
    let result = parse_with(
        LuaParser::new().unwrap(),
        "-- Line comment\n--[[ Block comment ]]\n-- Another comment\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        LuaParser::new().unwrap(),
        "local M = {}\r\nfunction M.hello() return true end\r\nreturn M\r\n",
    );

    assert_exports_include(&result, &["hello"]);
}

#[test]
fn mixed_module_and_global() {
    let source = "local M = {}\nfunction M.foo() end\nfunction bar() end\nlocal function baz() end\nreturn M\n";

    let result = parse_with(LuaParser::new().unwrap(), source);

    assert_exports_include(&result, &["foo", "bar"]);
    assert_exports_exclude(&result, &["baz"]);
    assert_eq!(result.metadata.export_names().len(), 2);
}
