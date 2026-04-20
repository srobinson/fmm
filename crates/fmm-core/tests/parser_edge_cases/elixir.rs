use super::support::{
    assert_empty_parse, assert_exports_exclude, assert_exports_include, assert_no_exports,
    assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::elixir::ElixirParser;

#[test]
fn empty_file() {
    assert_empty_parse(ElixirParser::new().unwrap());
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(ElixirParser::new().unwrap(), "def {{{{ invalid !@#$");
}

#[test]
fn no_exports() {
    assert_no_exports(ElixirParser::new().unwrap(), "# just a comment\n1 + 2\n");
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        ElixirParser::new().unwrap(),
        "defmodule M do\r\n  def hello() do\r\n    :ok\r\n  end\r\nend\r\n",
    );

    assert_exports_include(&result, &["M", "hello"]);
}

#[test]
fn private_excluded() {
    let source = "defmodule M do\n  def public(), do: :ok\n  defp private(), do: :ok\n  defmacro pub_macro(), do: :ok\n  defmacrop priv_macro(), do: :ok\nend\n";

    let result = parse_with(ElixirParser::new().unwrap(), source);

    assert_exports_include(&result, &["public", "pub_macro"]);
    assert_exports_exclude(&result, &["private", "priv_macro"]);
}

#[test]
fn whitespace_only() {
    assert_no_exports(ElixirParser::new().unwrap(), "   \n\n  \n");
}
