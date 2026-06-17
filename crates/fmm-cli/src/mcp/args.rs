use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LookupExportArgs {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListExportsArgs {
    pub(super) pattern: Option<String>,
    pub(super) file: Option<String>,
    pub(super) directory: Option<String>,
    pub(super) filter: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DependencyGraphArgs {
    pub(super) file: String,
    pub(super) depth: Option<i32>,
    /// source / tests / all (default)
    pub(super) filter: Option<String>,
    pub(super) reverse: Option<bool>,
    pub(super) transitive: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DependencyCyclesArgs {
    pub(super) file: Option<String>,
    /// source / tests / all (default)
    pub(super) filter: Option<String>,
    /// runtime / all (default: runtime)
    pub(super) edge_mode: Option<String>,
    /// Include module-hierarchy facade edges.
    pub(super) include_mod_hierarchy: Option<bool>,
    /// Include edges that keep each SCC connected.
    pub(super) explain: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchArgs {
    pub(super) term: Option<String>,
    pub(super) export: Option<String>,
    pub(super) imports: Option<String>,
    pub(super) depends_on: Option<String>,
    pub(super) min_loc: Option<usize>,
    pub(super) max_loc: Option<usize>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadSymbolArgs {
    pub(super) name: String,
    /// When false, bypasses the 10KB response cap (default: true).
    pub(super) truncate: Option<bool>,
    /// When true, prepend absolute line numbers to each source line (ALP-829).
    pub(super) line_numbers: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileOutlineArgs {
    pub(super) file: String,
    /// When true, add on-demand private members and non-exported top-level declarations.
    pub(super) include_private: Option<bool>,
    /// When false, bypasses the 10KB MCP response cap (default: true).
    #[allow(dead_code)]
    pub(super) truncate: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListFilesArgs {
    pub(super) directory: Option<String>,
    pub(super) pattern: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) offset: Option<usize>,
    pub(super) sort_by: Option<String>,
    pub(super) order: Option<String>,
    pub(super) group_by: Option<String>,
    pub(super) filter: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GlossaryArgs {
    pub(super) pattern: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) mode: Option<String>,
    /// ALP-883: "named" (default) = Layer 2 only; "call-site" = Layer 2 + Layer 3 tree-sitter.
    pub(super) precision: Option<String>,
    pub(super) exact: Option<bool>,
    /// When false, bypasses the 10KB response cap (default: true).
    /// Read from raw JSON in MCP dispatch; field exists so serde accepts the parameter.
    #[allow(dead_code)]
    pub(super) truncate: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FindSimilarArgs {
    pub(super) name: String,
    pub(super) signature: Option<String>,
    pub(super) kind: Option<String>,
    pub(super) directory: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) include_tests: Option<bool>,
}

#[derive(Deserialize)]
pub(super) struct DupeClustersArgs {
    pub(super) directory: Option<String>,
    pub(super) kind: Option<Vec<String>>,
    pub(super) min_score: Option<f64>,
    pub(super) limit: Option<usize>,
    pub(super) include_tests: Option<bool>,
}
