//! Parser corpus validation against realistic cross language source snippets.
//!
//! These tests live in fmm-core because they exercise parser metadata directly.
//! Snippets are stored as fixtures so corpus changes are separate from expectation changes.

#[path = "parser_cross_language/support.rs"]
mod support;

#[path = "parser_cross_language/python.rs"]
mod python;

#[path = "parser_cross_language/rust.rs"]
mod rust;

#[path = "parser_cross_language/typescript.rs"]
mod typescript;

#[path = "parser_cross_language/go.rs"]
mod go;

#[path = "parser_cross_language/java.rs"]
mod java;

#[path = "parser_cross_language/cpp.rs"]
mod cpp;

#[path = "parser_cross_language/csharp.rs"]
mod csharp;

#[path = "parser_cross_language/ruby.rs"]
mod ruby;

#[path = "parser_cross_language/php.rs"]
mod php;

#[path = "parser_cross_language/c.rs"]
mod c;

#[path = "parser_cross_language/zig.rs"]
mod zig;

#[path = "parser_cross_language/lua.rs"]
mod lua;

#[path = "parser_cross_language/scala.rs"]
mod scala;

#[path = "parser_cross_language/swift.rs"]
mod swift;

#[path = "parser_cross_language/kotlin.rs"]
mod kotlin;

#[path = "parser_cross_language/dart.rs"]
mod dart;

#[path = "parser_cross_language/elixir.rs"]
mod elixir;
