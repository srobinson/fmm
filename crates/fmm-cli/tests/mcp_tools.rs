//! MCP tool integration tests.
//!
//! Every module calls through `SqliteMcpServer::call_tool`, the real JSON-RPC
//! tool path. Shared fixtures generate SQLite indexes in temp dirs so the
//! server loads realistic manifests with line ranges.

#[path = "mcp_tools/dependency_graph.rs"]
mod dependency_graph;
#[path = "mcp_tools/file_outline.rs"]
mod file_outline;
#[path = "mcp_tools/glossary.rs"]
mod glossary;
#[path = "mcp_tools/go_resolution.rs"]
mod go_resolution;
#[path = "mcp_tools/list_exports.rs"]
mod list_exports;
#[path = "mcp_tools/lookup_export.rs"]
mod lookup_export;
#[path = "mcp_tools/manifest.rs"]
mod manifest;
#[path = "mcp_tools/read_symbol.rs"]
mod read_symbol;
#[path = "mcp_tools/search_combined_filters.rs"]
mod search_combined_filters;
#[path = "mcp_tools/search_export.rs"]
mod search_export;
#[path = "mcp_tools/search_filters.rs"]
mod search_filters;
#[path = "mcp_tools/search_multi_filters.rs"]
mod search_multi_filters;
#[path = "mcp_tools/search_terms.rs"]
mod search_terms;
#[path = "mcp_tools/support.rs"]
mod support;
