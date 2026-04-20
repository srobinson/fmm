//! MCP tool implementations — one file per tool.

pub(super) mod common;
mod exports;
mod glossary;
mod graph;
mod list_files;
mod lookup;
mod outline;
mod read;
mod search;

// Tool dispatch — re-exported for use from mcp/mod.rs
pub(super) use exports::tool_list_exports;
pub(super) use glossary::tool_glossary;
pub(super) use graph::tool_dependency_graph;
pub(super) use list_files::tool_list_files;
pub(super) use lookup::tool_lookup_export;
pub(super) use outline::tool_file_outline;
pub(super) use read::tool_read_symbol;
pub(super) use search::tool_search;

// Shared utilities re-exported for mcp/tests and cli/navigate.rs
pub(crate) use common::{compute_import_specifiers, find_concrete_definition, is_reexport_file};
// Re-exported for mcp/tests (only visible in test builds, so suppress the lint)
#[allow(unused_imports)]
pub(super) use common::glob_filename_matches;
