pub mod cli;
pub mod config;
pub mod db;
pub mod extractor;
pub mod format;
pub mod manifest;
pub mod mcp;
pub mod resolver;
pub mod search;

// Re-export fmm-core's parser module so internal `crate::parser::*` references
// continue to work without changing every import site. Consumers gradually
// migrate to `fmm_core::parser::*` as modules move to fmm-core.
pub use fmm_core::parser;
