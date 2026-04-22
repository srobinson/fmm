pub mod cli;
pub mod fs_utils;
pub(crate) mod glossary;
pub mod mcp;
pub(crate) mod read_symbol;

/// CLI version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("FMM_VERSION");
