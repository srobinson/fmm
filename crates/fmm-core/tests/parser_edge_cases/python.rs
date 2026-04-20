use super::support::{
    assert_empty_parse, assert_exports_include, assert_no_exports, assert_parse_ok, parse_with,
};
use fmm_core::parser::builtin::python::PythonParser;

#[test]
fn empty_file() {
    assert_empty_parse(PythonParser::new().unwrap());
}

#[test]
fn huge_file() {
    let mut lines = vec!["import os".to_string()];
    for i in 0..10_000 {
        lines.push(format!("def func_{}():\n    pass", i));
    }

    let result = parse_with(PythonParser::new().unwrap(), &lines.join("\n"));

    assert!(result.metadata.exports.len() > 1000);
    assert!(result.metadata.loc > 10_000);
}

#[test]
fn syntax_errors_are_tolerated() {
    assert_parse_ok(
        PythonParser::new().unwrap(),
        "def !!!invalid:\n    return @@@",
    );
}

#[test]
fn underscore_top_level_defs_are_exported() {
    let source = "def _private():\n    pass\n\n_INTERNAL = 42\n";

    let result = parse_with(PythonParser::new().unwrap(), source);

    assert_exports_include(&result, &["_private", "_INTERNAL"]);
}

#[test]
fn unicode_identifiers() {
    let result = parse_with(
        PythonParser::new().unwrap(),
        "def grüße():\n    pass\n\nclass Ñoño:\n    pass\n",
    );

    assert!(!result.metadata.exports.is_empty());
}

#[test]
fn crlf_line_endings() {
    let result = parse_with(
        PythonParser::new().unwrap(),
        "def hello():\r\n    pass\r\n\r\nclass World:\r\n    pass\r\n",
    );

    assert_exports_include(&result, &["hello", "World"]);
}

#[test]
fn whitespace_only() {
    assert_no_exports(PythonParser::new().unwrap(), "   \n\n  \t  \n");
}

#[test]
fn comments_only() {
    let result = parse_with(
        PythonParser::new().unwrap(),
        "# Just a comment\n# Another comment\n",
    );

    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 2);
}

#[test]
fn decorated_underscore_items_are_exported() {
    let source =
        "@dataclass\nclass _Internal:\n    x: int\n\n@staticmethod\ndef _helper():\n    pass\n";

    let result = parse_with(PythonParser::new().unwrap(), source);

    assert_exports_include(&result, &["_Internal", "_helper"]);
}

#[test]
fn decorated_crlf_line_endings() {
    let result = parse_with(
        PythonParser::new().unwrap(),
        "@dataclass\r\nclass Agent:\r\n    name: str\r\n",
    );

    assert_exports_include(&result, &["Agent"]);
    let agent = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "Agent")
        .unwrap();
    assert_eq!(agent.start_line, 1, "range should start at decorator");
}

#[test]
fn stacked_decorators() {
    let source = "@decorator_a\n@decorator_b\n@decorator_c\ndef stacked():\n    pass\n";

    let result = parse_with(PythonParser::new().unwrap(), source);

    assert_exports_include(&result, &["stacked"]);
    let entry = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "stacked")
        .unwrap();
    assert_eq!(entry.start_line, 1, "range should include first decorator");
    assert_eq!(entry.end_line, 5);
}

#[test]
fn decorated_mixed_with_bare() {
    let source = "def bare():\n    pass\n\n@app.route(\"/\")\ndef decorated():\n    pass\n\nclass Plain:\n    pass\n\n@dataclass\nclass Fancy:\n    x: int\n";

    let result = parse_with(PythonParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    assert_eq!(names.len(), 4);
    assert!(names.contains(&"bare".to_string()));
    assert!(names.contains(&"decorated".to_string()));
    assert!(names.contains(&"Plain".to_string()));
    assert!(names.contains(&"Fancy".to_string()));
}
