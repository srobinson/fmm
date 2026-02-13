use fmm::parser::builtin::cpp::CppParser;
use fmm::parser::builtin::csharp::CSharpParser;
use fmm::parser::builtin::go::GoParser;
use fmm::parser::builtin::java::JavaParser;
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::ruby::RubyParser;
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
    assert!(result
        .metadata
        .export_names()
        .contains(&"hello".to_string()));
}

// --- Windows line endings (CRLF) ---

#[test]
fn typescript_crlf_line_endings() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "export function hello() {}\r\nexport const world = 42;\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result
        .metadata
        .export_names()
        .contains(&"hello".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"world".to_string()));
}

#[test]
fn python_crlf_line_endings() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def hello():\r\n    pass\r\n\r\nclass World:\r\n    pass\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result
        .metadata
        .export_names()
        .contains(&"hello".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"World".to_string()));
}

#[test]
fn rust_crlf_line_endings() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() {}\r\npub struct World {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result
        .metadata
        .export_names()
        .contains(&"hello".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"World".to_string()));
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

// =============================================================================
// Go edge cases
// =============================================================================

#[test]
fn go_empty_file() {
    let mut parser = GoParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn go_syntax_errors() {
    let mut parser = GoParser::new().unwrap();
    let source = "package main\n\nfunc {{{{ invalid";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn go_no_exports() {
    let mut parser = GoParser::new().unwrap();
    let source = "package main\n\nfunc private() {}\nvar internal = 1\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn go_comments_only() {
    let mut parser = GoParser::new().unwrap();
    let source = "// Just a comment\n/* Block comment */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

// =============================================================================
// Java edge cases
// =============================================================================

#[test]
fn java_empty_file() {
    let mut parser = JavaParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn java_syntax_errors() {
    let mut parser = JavaParser::new().unwrap();
    let source = "public class {{{{ invalid }";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn java_comments_only() {
    let mut parser = JavaParser::new().unwrap();
    let source = "// Comment\n/** Javadoc */\n/* Block */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

// =============================================================================
// C++ edge cases
// =============================================================================

#[test]
fn cpp_empty_file() {
    let mut parser = CppParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn cpp_syntax_errors() {
    let mut parser = CppParser::new().unwrap();
    let source = "class {{{{ not valid }";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn cpp_comments_only() {
    let mut parser = CppParser::new().unwrap();
    let source = "// Comment\n/* Block */\n/// Doc\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn cpp_crlf_line_endings() {
    let mut parser = CppParser::new().unwrap();
    let source = "#include <vector>\r\nclass Foo {};\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"vector".to_string()));
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
}

// =============================================================================
// C# edge cases
// =============================================================================

#[test]
fn csharp_empty_file() {
    let mut parser = CSharpParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn csharp_syntax_errors() {
    let mut parser = CSharpParser::new().unwrap();
    let source = "public class {{{{ not valid }";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn csharp_comments_only() {
    let mut parser = CSharpParser::new().unwrap();
    let source = "// Comment\n/// Doc comment\n/* Block */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

// =============================================================================
// Ruby edge cases
// =============================================================================

#[test]
fn ruby_empty_file() {
    let mut parser = RubyParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn ruby_syntax_errors() {
    let mut parser = RubyParser::new().unwrap();
    let source = "class !!!! end";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn ruby_comments_only() {
    let mut parser = RubyParser::new().unwrap();
    let source = "# Just a comment\n# Another comment\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn ruby_no_exports_all_private() {
    let mut parser = RubyParser::new().unwrap();
    let source = "def _private\n  nil\nend\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}
