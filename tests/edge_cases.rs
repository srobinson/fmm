use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::typescript::TypeScriptParser;
use fmm::parser::Parser;

// --- Empty files ---

#[test]
fn typescript_empty_file() {
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn python_empty_file() {
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn rust_empty_file() {
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

// --- Huge files (10k+ lines) ---

#[test]
fn typescript_huge_file() {
    let mut lines = vec!["import { Foo } from 'bar';".to_string()];
    for i in 0..10_000 {
        lines.push(format!("export const x{} = {};", i, i));
    }
    let source = lines.join("\n");
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(&source).unwrap();
    assert!(result.metadata.exports.len() > 1000);
    assert_eq!(result.metadata.loc, 10_001);
}

#[test]
fn python_huge_file() {
    let mut lines = vec!["import os".to_string()];
    for i in 0..10_000 {
        lines.push(format!("def func_{}():\n    pass", i));
    }
    let source = lines.join("\n");
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(&source).unwrap();
    assert!(result.metadata.exports.len() > 1000);
    assert!(result.metadata.loc > 10_000);
}

#[test]
fn rust_huge_file() {
    let mut lines = vec!["use std::io;".to_string()];
    for i in 0..10_000 {
        lines.push(format!("pub fn func_{}() {{}}", i));
    }
    let source = lines.join("\n");
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(&source).unwrap();
    assert!(result.metadata.exports.len() > 1000);
    assert_eq!(result.metadata.loc, 10_001);
}

// --- Syntax errors (invalid code) ---

#[test]
fn typescript_syntax_errors() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "export function {{{{ invalid syntax !@#$";
    // tree-sitter is error-tolerant — should parse without panicking
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn python_syntax_errors() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def !!!invalid:\n    return @@@";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn rust_syntax_errors() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn {{{{ not valid rust }}}}";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

// --- No exports (internal module) ---

#[test]
fn typescript_no_exports() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "const internal = 42;\nfunction helper() { return internal; }";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn python_no_exports_all_private() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def _private():\n    pass\n\n_INTERNAL = 42\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn rust_no_exports() {
    let mut parser = RustParser::new().unwrap();
    let source = "fn private_fn() {}\nstruct Internal {}";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

// --- Unicode in identifiers ---

#[test]
fn python_unicode_identifiers() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def grüße():\n    pass\n\nclass Ñoño:\n    pass\n";
    let result = parser.parse(source);
    assert!(result.is_ok());
    let result = result.unwrap();
    // Python supports unicode identifiers
    assert!(!result.metadata.exports.is_empty());
}

#[test]
fn rust_unicode_in_strings() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() -> &'static str { \"こんにちは\" }";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.contains(&"hello".to_string()));
}

// --- Windows line endings (CRLF) ---

#[test]
fn typescript_crlf_line_endings() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "export function hello() {}\r\nexport const world = 42;\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.contains(&"hello".to_string()));
    assert!(result.metadata.exports.contains(&"world".to_string()));
}

#[test]
fn python_crlf_line_endings() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def hello():\r\n    pass\r\n\r\nclass World:\r\n    pass\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.contains(&"hello".to_string()));
    assert!(result.metadata.exports.contains(&"World".to_string()));
}

#[test]
fn rust_crlf_line_endings() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() {}\r\npub struct World {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.contains(&"hello".to_string()));
    assert!(result.metadata.exports.contains(&"World".to_string()));
}

// --- Whitespace-only files ---

#[test]
fn typescript_whitespace_only() {
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse("   \n\n  \t  \n").unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn python_whitespace_only() {
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse("   \n\n  \t  \n").unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn rust_whitespace_only() {
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse("   \n\n  \t  \n").unwrap();
    assert!(result.metadata.exports.is_empty());
}

// --- Comments-only files ---

#[test]
fn typescript_comments_only() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "// Just a comment\n/* Block comment */\n// Another comment\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn python_comments_only() {
    let mut parser = PythonParser::new().unwrap();
    let source = "# Just a comment\n# Another comment\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 2);
}

#[test]
fn rust_comments_only() {
    let mut parser = RustParser::new().unwrap();
    let source = "// Just a comment\n/// Doc comment\n//! Module doc\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}
