//! Parser validation against checked in language fixtures.
//!
//! These tests live in fmm-core because they exercise parser metadata directly.
//! Each module owns one language fixture surface so expectations stay local.

pub use fmm_core::parser::Parser;
pub use fmm_core::parser::builtin::c::CParser;
pub use fmm_core::parser::builtin::cpp::CppParser;
pub use fmm_core::parser::builtin::csharp::CSharpParser;
pub use fmm_core::parser::builtin::dart::DartParser;
pub use fmm_core::parser::builtin::elixir::ElixirParser;
pub use fmm_core::parser::builtin::go::GoParser;
pub use fmm_core::parser::builtin::java::JavaParser;
pub use fmm_core::parser::builtin::kotlin::KotlinParser;
pub use fmm_core::parser::builtin::lua::LuaParser;
pub use fmm_core::parser::builtin::php::PhpParser;
pub use fmm_core::parser::builtin::python::PythonParser;
pub use fmm_core::parser::builtin::ruby::RubyParser;
pub use fmm_core::parser::builtin::rust::RustParser;
pub use fmm_core::parser::builtin::scala::ScalaParser;
pub use fmm_core::parser::builtin::swift::SwiftParser;
pub use fmm_core::parser::builtin::typescript::TypeScriptParser;
pub use fmm_core::parser::builtin::zig::ZigParser;

#[path = "fixture_validation/support.rs"]
mod support;
pub use support::{assert_exports_sorted, parse_fixture};

#[path = "fixture_validation/python.rs"]
mod python;

#[path = "fixture_validation/rust.rs"]
mod rust;

#[path = "fixture_validation/go.rs"]
mod go;

#[path = "fixture_validation/java.rs"]
mod java;

#[path = "fixture_validation/cpp.rs"]
mod cpp;

#[path = "fixture_validation/csharp.rs"]
mod csharp;

#[path = "fixture_validation/ruby.rs"]
mod ruby;

#[path = "fixture_validation/php.rs"]
mod php;

#[path = "fixture_validation/c.rs"]
mod c;

#[path = "fixture_validation/zig.rs"]
mod zig;

#[path = "fixture_validation/lua.rs"]
mod lua;

#[path = "fixture_validation/scala.rs"]
mod scala;

#[path = "fixture_validation/swift.rs"]
mod swift;

#[path = "fixture_validation/kotlin.rs"]
mod kotlin;

#[path = "fixture_validation/dart.rs"]
mod dart;

#[path = "fixture_validation/elixir.rs"]
mod elixir;

#[path = "fixture_validation/typescript.rs"]
mod typescript;

#[path = "fixture_validation/template.rs"]
mod template;
