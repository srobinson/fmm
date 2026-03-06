use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct LookupExportArgs {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListExportsArgs {
    pub(super) pattern: Option<String>,
    pub(super) file: Option<String>,
    pub(super) directory: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DependencyGraphArgs {
    pub(super) file: String,
    pub(super) depth: Option<i32>,
}

#[derive(Debug, Deserialize)]
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
pub(super) struct ReadSymbolArgs {
    pub(super) name: String,
    /// When false, bypasses the 10KB response cap (default: true).
    pub(super) truncate: Option<bool>,
    /// When true, prepend absolute line numbers to each source line (ALP-829).
    pub(super) line_numbers: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FileOutlineArgs {
    pub(super) file: String,
    /// When true, include private/protected members in the outline (ALP-827).
    pub(super) include_private: Option<bool>,
}

#[derive(Debug, Deserialize)]
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
pub(super) struct GlossaryArgs {
    pub(super) pattern: Option<String>,
    pub(super) limit: Option<usize>,
    pub(super) mode: Option<String>,
}
