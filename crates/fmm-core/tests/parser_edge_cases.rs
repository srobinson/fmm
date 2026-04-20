//! Parser edge case regression coverage.
//!
//! These tests live in fmm-core because they exercise parser metadata directly.
//! Each module owns one language surface so edge behavior stays easy to scan.

#[path = "parser_edge_cases/support.rs"]
mod support;

#[path = "parser_edge_cases/typescript.rs"]
mod typescript;

#[path = "parser_edge_cases/python.rs"]
mod python;

#[path = "parser_edge_cases/rust.rs"]
mod rust;

#[path = "parser_edge_cases/go.rs"]
mod go;

#[path = "parser_edge_cases/java.rs"]
mod java;

#[path = "parser_edge_cases/cpp.rs"]
mod cpp;

#[path = "parser_edge_cases/csharp.rs"]
mod csharp;

#[path = "parser_edge_cases/ruby.rs"]
mod ruby;

#[path = "parser_edge_cases/php.rs"]
mod php;

#[path = "parser_edge_cases/c.rs"]
mod c;

#[path = "parser_edge_cases/zig.rs"]
mod zig;

#[path = "parser_edge_cases/lua.rs"]
mod lua;

#[path = "parser_edge_cases/scala.rs"]
mod scala;

#[path = "parser_edge_cases/swift.rs"]
mod swift;

#[path = "parser_edge_cases/kotlin.rs"]
mod kotlin;

#[path = "parser_edge_cases/dart.rs"]
mod dart;

#[path = "parser_edge_cases/elixir.rs"]
mod elixir;
