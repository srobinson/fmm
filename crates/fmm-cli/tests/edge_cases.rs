use fmm_core::parser::Parser;
use fmm_core::parser::builtin::c::CParser;
use fmm_core::parser::builtin::cpp::CppParser;
use fmm_core::parser::builtin::csharp::CSharpParser;
use fmm_core::parser::builtin::dart::DartParser;
use fmm_core::parser::builtin::elixir::ElixirParser;
use fmm_core::parser::builtin::go::GoParser;
use fmm_core::parser::builtin::java::JavaParser;
use fmm_core::parser::builtin::kotlin::KotlinParser;
use fmm_core::parser::builtin::lua::LuaParser;
use fmm_core::parser::builtin::php::PhpParser;
use fmm_core::parser::builtin::python::PythonParser;
use fmm_core::parser::builtin::ruby::RubyParser;
use fmm_core::parser::builtin::rust::RustParser;
use fmm_core::parser::builtin::scala::ScalaParser;
use fmm_core::parser::builtin::swift::SwiftParser;
use fmm_core::parser::builtin::typescript::TypeScriptParser;
use fmm_core::parser::builtin::zig::ZigParser;

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
fn python_underscore_top_level_defs_are_exported() {
    // Underscore-prefix is Python social convention, not a structural property.
    // fmm surfaces all top-level defs so cross-module re-export dereferencing
    // works (e.g. `_port_in_use` defined in `net.py` and re-exported from a
    // barrel `__init__.py`).
    let mut parser = PythonParser::new().unwrap();
    let source = "def _private():\n    pass\n\n_INTERNAL = 42\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"_private".to_string()));
    assert!(names.contains(&"_INTERNAL".to_string()));
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
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
}

// --- Windows line endings (CRLF) ---

#[test]
fn typescript_crlf_line_endings() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = "export function hello() {}\r\nexport const world = 42;\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"world".to_string())
    );
}

#[test]
fn python_crlf_line_endings() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def hello():\r\n    pass\r\n\r\nclass World:\r\n    pass\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"World".to_string())
    );
}

#[test]
fn rust_crlf_line_endings() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() {}\r\npub struct World {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"World".to_string())
    );
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

// --- Decorated definitions (Python-specific) ---

#[test]
fn python_decorated_underscore_items_are_exported() {
    // Matching `python_underscore_top_level_defs_are_exported`: underscore
    // prefix is not a structural filter, even for decorated items.
    let mut parser = PythonParser::new().unwrap();
    let source =
        "@dataclass\nclass _Internal:\n    x: int\n\n@staticmethod\ndef _helper():\n    pass\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"_Internal".to_string()));
    assert!(names.contains(&"_helper".to_string()));
}

#[test]
fn python_decorated_crlf_line_endings() {
    let mut parser = PythonParser::new().unwrap();
    let source = "@dataclass\r\nclass Agent:\r\n    name: str\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Agent".to_string())
    );
    let agent = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "Agent")
        .unwrap();
    assert_eq!(agent.start_line, 1, "range should start at decorator");
}

#[test]
fn python_stacked_decorators() {
    let mut parser = PythonParser::new().unwrap();
    let source = "@decorator_a\n@decorator_b\n@decorator_c\ndef stacked():\n    pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"stacked".to_string())
    );
    let entry = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "stacked")
        .unwrap();
    assert_eq!(entry.start_line, 1, "range should include first decorator");
    assert_eq!(entry.end_line, 5);
}

#[test]
fn python_decorated_mixed_with_bare() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def bare():\n    pass\n\n@app.route(\"/\")\ndef decorated():\n    pass\n\nclass Plain:\n    pass\n\n@dataclass\nclass Fancy:\n    x: int\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"bare".to_string()));
    assert!(names.contains(&"decorated".to_string()));
    assert!(names.contains(&"Plain".to_string()));
    assert!(names.contains(&"Fancy".to_string()));
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

// --- TypeScript default export edge cases ---

#[test]
fn typescript_default_anonymous_arrow_not_captured() {
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse("export default () => {};").unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn typescript_default_anonymous_class_not_captured() {
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse("export default class { run() {} }").unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn typescript_default_object_literal_not_captured() {
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser
        .parse("export default { key: 'value', num: 42 };")
        .unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn typescript_default_and_named_coexist() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = r#"
export const helper = () => {};
export function util() {}
export default function Main() {}
"#;
    let result = parser.parse(source).unwrap();
    assert_eq!(
        result.metadata.export_names(),
        vec!["helper", "util", "Main"]
    );
}

#[test]
fn typescript_type_only_exports() {
    let mut parser = TypeScriptParser::new().unwrap();
    let source = r#"
export type A = string;
export type B = number;
export type C = A | B;
"#;
    let result = parser.parse(source).unwrap();
    assert_eq!(result.metadata.export_names(), vec!["A", "B", "C"]);
}

#[test]
fn typescript_default_identifier_deduplicates_with_named() {
    let mut parser = TypeScriptParser::new().unwrap();
    // If Component is both in an export clause and a default, it should appear once
    let source = "export { Component };\nexport default Component;";
    let result = parser.parse(source).unwrap();
    assert_eq!(result.metadata.export_names(), vec!["Component"]);
}

// =============================================================================
// PHP edge cases
// =============================================================================

#[test]
fn php_empty_file() {
    let mut parser = PhpParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn php_syntax_errors() {
    let mut parser = PhpParser::new().unwrap();
    let source = "<?php\nclass {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn php_no_exports() {
    let mut parser = PhpParser::new().unwrap();
    // PHP file with only private/protected members
    let source =
        "<?php\nclass Foo {\n    private function bar() {}\n    protected function baz() {}\n}\n";
    let result = parser.parse(source).unwrap();
    // Class itself is still exported, but private/protected methods are not
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
    assert!(!result.metadata.export_names().contains(&"bar".to_string()));
    assert!(!result.metadata.export_names().contains(&"baz".to_string()));
}

#[test]
fn php_comments_only() {
    let mut parser = PhpParser::new().unwrap();
    let source = "<?php\n// Just a comment\n/* Block comment */\n/** Docblock */\n";
    let result = parser.parse(source).unwrap();
    // No classes/functions/constants
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn php_crlf_line_endings() {
    let mut parser = PhpParser::new().unwrap();
    let source = "<?php\r\nclass Foo {}\r\nfunction bar() {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
    assert!(result.metadata.export_names().contains(&"bar".to_string()));
}

#[test]
fn php_enum_with_methods() {
    let mut parser = PhpParser::new().unwrap();
    let source = "<?php\nenum Color {\n    case Red;\n    case Blue;\n    public function label(): string { return $this->name; }\n}\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Color".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"label".to_string())
    );
}

// =============================================================================
// C edge cases
// =============================================================================

#[test]
fn c_empty_file() {
    let mut parser = CParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn c_syntax_errors() {
    let mut parser = CParser::new().unwrap();
    let source = "int {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn c_no_exports() {
    let mut parser = CParser::new().unwrap();
    let source = "static int helper() { return 0; }\nstatic void internal() {}\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn c_comments_only() {
    let mut parser = CParser::new().unwrap();
    let source = "/* Block comment */\n// Line comment\n/* Another block */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn c_crlf_line_endings() {
    let mut parser = CParser::new().unwrap();
    let source = "#include <stdio.h>\r\nint main() { return 0; }\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
    assert!(result.metadata.export_names().contains(&"main".to_string()));
}

#[test]
fn c_header_only_macros() {
    let mut parser = CParser::new().unwrap();
    let source =
        "#ifndef GUARD_H\n#define GUARD_H\n#define MAX 100\n#define VERSION \"1.0\"\n#endif\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"GUARD_H".to_string()));
    assert!(names.contains(&"MAX".to_string()));
    assert!(names.contains(&"VERSION".to_string()));
}

// =============================================================================
// Zig edge cases
// =============================================================================

#[test]
fn zig_empty_file() {
    let mut parser = ZigParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn zig_syntax_errors() {
    let mut parser = ZigParser::new().unwrap();
    let source = "pub fn {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn zig_no_exports() {
    let mut parser = ZigParser::new().unwrap();
    let source = "const internal: u32 = 42;\nfn private() void {}\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn zig_comments_only() {
    let mut parser = ZigParser::new().unwrap();
    let source = "// Just a comment\n/// Doc comment\n//! Module doc\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn zig_crlf_line_endings() {
    let mut parser = ZigParser::new().unwrap();
    let source = "const std = @import(\"std\");\r\npub fn hello() void {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
}

#[test]
fn zig_error_set() {
    let mut parser = ZigParser::new().unwrap();
    let source = "pub const MyError = error{ Foo, Bar, Baz };\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"MyError".to_string())
    );
}

// =============================================================================
// Lua edge cases
// =============================================================================

#[test]
fn lua_empty_file() {
    let mut parser = LuaParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn lua_syntax_errors() {
    let mut parser = LuaParser::new().unwrap();
    let source = "function {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn lua_no_exports() {
    let mut parser = LuaParser::new().unwrap();
    let source = "local function helper() return true end\nlocal x = 42\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn lua_comments_only() {
    let mut parser = LuaParser::new().unwrap();
    let source = "-- Line comment\n--[[ Block comment ]]\n-- Another comment\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn lua_crlf_line_endings() {
    let mut parser = LuaParser::new().unwrap();
    let source = "local M = {}\r\nfunction M.hello() return true end\r\nreturn M\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
}

#[test]
fn lua_mixed_module_and_global() {
    let mut parser = LuaParser::new().unwrap();
    let source = "local M = {}\nfunction M.foo() end\nfunction bar() end\nlocal function baz() end\nreturn M\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"foo".to_string()));
    assert!(names.contains(&"bar".to_string()));
    assert!(!names.contains(&"baz".to_string()));
    assert_eq!(names.len(), 2);
}

// =============================================================================
// Scala edge cases
// =============================================================================

#[test]
fn scala_empty_file() {
    let mut parser = ScalaParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn scala_syntax_errors() {
    let mut parser = ScalaParser::new().unwrap();
    let source = "class {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn scala_no_exports() {
    let mut parser = ScalaParser::new().unwrap();
    let source = "private class Foo\nprivate object Bar\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn scala_comments_only() {
    let mut parser = ScalaParser::new().unwrap();
    let source = "// Line comment\n/* Block comment */\n/** Scaladoc */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn scala_crlf_line_endings() {
    let mut parser = ScalaParser::new().unwrap();
    let source = "import scala.io\r\nclass Foo\r\nobject Bar\r\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"scala".to_string()));
    let names = result.metadata.export_names();
    assert!(names.contains(&"Foo".to_string()));
    assert!(names.contains(&"Bar".to_string()));
}

#[test]
fn scala_protected_excluded() {
    let mut parser = ScalaParser::new().unwrap();
    let source = "protected class Foo\nclass Bar\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(!names.contains(&"Foo".to_string()));
    assert!(names.contains(&"Bar".to_string()));
}

// =============================================================================
// Swift edge cases
// =============================================================================

#[test]
fn swift_empty_file() {
    let mut parser = SwiftParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn swift_syntax_errors() {
    let mut parser = SwiftParser::new().unwrap();
    let source = "public func {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn swift_no_exports() {
    let mut parser = SwiftParser::new().unwrap();
    let source = "private func helper() {}\ninternal struct Config {}\nfunc defaultFunc() {}\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn swift_comments_only() {
    let mut parser = SwiftParser::new().unwrap();
    let source = "// Line comment\n/* Block comment */\n/// Doc comment\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn swift_crlf_line_endings() {
    let mut parser = SwiftParser::new().unwrap();
    let source = "import Foundation\r\npublic func hello() {}\r\npublic struct Point {}\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Point".to_string())
    );
}

#[test]
fn swift_mixed_visibility() {
    let mut parser = SwiftParser::new().unwrap();
    let source = "public func visible() {}\nprivate func hidden() {}\nfileprivate func alsoHidden() {}\nopen class Base {}\ninternal class NotExported {}\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"visible".to_string()));
    assert!(names.contains(&"Base".to_string()));
    assert!(!names.contains(&"hidden".to_string()));
    assert!(!names.contains(&"alsoHidden".to_string()));
    assert!(!names.contains(&"NotExported".to_string()));
    assert_eq!(names.len(), 2);
}

// =============================================================================
// Kotlin edge cases
// =============================================================================

#[test]
fn kotlin_empty_file() {
    let mut parser = KotlinParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn kotlin_syntax_errors() {
    let mut parser = KotlinParser::new().unwrap();
    let source = "fun {{{{ invalid syntax !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn kotlin_no_exports() {
    let mut parser = KotlinParser::new().unwrap();
    let source = "private fun helper() {}\ninternal class Config\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn kotlin_comments_only() {
    let mut parser = KotlinParser::new().unwrap();
    let source = "// Line comment\n/* Block comment */\n/** KDoc comment */\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn kotlin_crlf_line_endings() {
    let mut parser = KotlinParser::new().unwrap();
    let source = "import kotlin.collections.List\r\nfun hello() {}\r\nclass MyClass\r\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"MyClass".to_string())
    );
}

#[test]
fn kotlin_default_public_vs_private() {
    let mut parser = KotlinParser::new().unwrap();
    let source = "fun visible() {}\nprivate fun hidden() {}\ninternal fun alsoHidden() {}\nclass MyClass\nprivate class Secret\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"visible".to_string()));
    assert!(names.contains(&"MyClass".to_string()));
    assert!(!names.contains(&"hidden".to_string()));
    assert!(!names.contains(&"alsoHidden".to_string()));
    assert!(!names.contains(&"Secret".to_string()));
    assert_eq!(names.len(), 2);
}

// ──────────────────────────────────────────────
// Dart edge cases
// ──────────────────────────────────────────────

#[test]
fn dart_empty_file() {
    let mut parser = DartParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn dart_syntax_errors() {
    let mut parser = DartParser::new().unwrap();
    let source = "class {{{{ invalid !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn dart_no_exports() {
    let mut parser = DartParser::new().unwrap();
    let source = "void _helper() {}\nclass _Internal {}\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn dart_crlf_line_endings() {
    let mut parser = DartParser::new().unwrap();
    let source = "import 'dart:io';\r\nvoid hello() {}\r\nclass MyClass {}\r\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"hello".to_string()));
    assert!(names.contains(&"MyClass".to_string()));
}

#[test]
fn dart_underscore_privacy() {
    let mut parser = DartParser::new().unwrap();
    let source = "class Public {}\nclass _Private {}\nvoid visible() {}\nvoid _hidden() {}\ntypedef Pub = void Function();\ntypedef _Priv = void Function();\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"Public".to_string()));
    assert!(names.contains(&"visible".to_string()));
    assert!(names.contains(&"Pub".to_string()));
    assert!(!names.contains(&"_Private".to_string()));
    assert!(!names.contains(&"_hidden".to_string()));
    assert!(!names.contains(&"_Priv".to_string()));
    assert_eq!(names.len(), 3);
}

#[test]
fn dart_whitespace_only() {
    let mut parser = DartParser::new().unwrap();
    let result = parser.parse("   \n\n  \n").unwrap();
    assert!(result.metadata.exports.is_empty());
}

// ──────────────────────────────────────────────
// Elixir edge cases
// ──────────────────────────────────────────────

#[test]
fn elixir_empty_file() {
    let mut parser = ElixirParser::new().unwrap();
    let result = parser.parse("").unwrap();
    assert!(result.metadata.exports.is_empty());
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
    assert_eq!(result.metadata.loc, 0);
}

#[test]
fn elixir_syntax_errors() {
    let mut parser = ElixirParser::new().unwrap();
    let source = "def {{{{ invalid !@#$";
    let result = parser.parse(source);
    assert!(result.is_ok());
}

#[test]
fn elixir_no_exports() {
    let mut parser = ElixirParser::new().unwrap();
    let source = "# just a comment\n1 + 2\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.exports.is_empty());
}

#[test]
fn elixir_crlf_line_endings() {
    let mut parser = ElixirParser::new().unwrap();
    let source = "defmodule M do\r\n  def hello() do\r\n    :ok\r\n  end\r\nend\r\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"M".to_string()));
    assert!(names.contains(&"hello".to_string()));
}

#[test]
fn elixir_private_excluded() {
    let mut parser = ElixirParser::new().unwrap();
    let source = "defmodule M do\n  def public(), do: :ok\n  defp private(), do: :ok\n  defmacro pub_macro(), do: :ok\n  defmacrop priv_macro(), do: :ok\nend\n";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"public".to_string()));
    assert!(names.contains(&"pub_macro".to_string()));
    assert!(!names.contains(&"private".to_string()));
    assert!(!names.contains(&"priv_macro".to_string()));
}

#[test]
fn elixir_whitespace_only() {
    let mut parser = ElixirParser::new().unwrap();
    let result = parser.parse("   \n\n  \n").unwrap();
    assert!(result.metadata.exports.is_empty());
}
