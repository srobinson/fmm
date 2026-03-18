// Template for new language contributors — gated with cfg(any()) so it is never
// compiled. Copy it to src/parser/builtin/<lang>.rs and follow docs/CONTRIBUTING_LANGUAGE.md.
#[cfg(any())]
pub mod template;

pub mod c;
pub mod cpp;
pub mod csharp;
pub mod dart;
pub mod elixir;
pub mod go;
pub mod java;
pub mod kotlin;
pub mod lua;
pub mod php;
pub mod python;
pub mod query_helpers;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod typescript;
pub mod zig;
