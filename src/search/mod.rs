//! Shared search logic for both CLI and MCP.
//!
//! Extracts the duplicated search algorithms from `cli/search.rs` and
//! `mcp/mod.rs` into reusable functions with typed result structs.

mod bare_search;
mod dependency_graph;
mod dependency_graph_transitive;
mod filter_search;
mod helpers;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A single export hit from a search.
pub struct ExportHit {
    pub name: String,
    pub file: String,
    pub lines: Option<[usize; 2]>,
}

/// A package import hit with all files that use it.
pub struct ImportHit {
    pub package: String,
    pub files: Vec<String>,
}

/// A named-import call-site hit: files that import `symbol` by name from `source`.
pub struct NamedImportHit {
    pub symbol: String,
    pub source: String,
    pub files: Vec<String>,
}

/// Result of a bare term search (grouped by type).
pub struct BareSearchResult {
    pub exports: Vec<ExportHit>,
    pub files: Vec<String>,
    pub imports: Vec<ImportHit>,
    /// Files that import the matched symbol by name from an external package.
    pub named_import_hits: Vec<NamedImportHit>,
    /// Total fuzzy export hits before the limit was applied. None = no limit applied.
    pub total_exports: Option<usize>,
}

/// Per-file search result for filter-based search.
pub struct FileSearchResult {
    pub file: String,
    pub exports: Vec<ExportHitCompact>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}

/// Export name + optional line range (used within FileSearchResult).
pub struct ExportHitCompact {
    pub name: String,
    pub lines: Option<[usize; 2]>,
}

/// Filters for structured search.
pub struct SearchFilters {
    pub export: Option<String>,
    pub imports: Option<String>,
    pub depends_on: Option<String>,
    pub min_loc: Option<usize>,
    pub max_loc: Option<usize>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default cap for fuzzy export results in bare_search.
pub const DEFAULT_SEARCH_LIMIT: usize = 50;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use bare_search::bare_search;
pub use dependency_graph::dependency_graph;
pub use dependency_graph_transitive::dependency_graph_transitive;
pub use filter_search::filter_search;
pub use helpers::find_export_matches;
