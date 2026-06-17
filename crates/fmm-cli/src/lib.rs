pub mod cli;
pub(crate) mod cycle_report;
pub(crate) mod filename_glob;
pub mod fs_utils;
pub(crate) mod git;
pub(crate) mod glossary;
pub mod mcp;
pub(crate) mod outline_freshness;
pub(crate) mod read_symbol;

/// CLI version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("FMM_VERSION");
