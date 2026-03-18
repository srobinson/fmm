pub mod config;
pub mod error;
pub mod extractor;
pub mod format;
pub mod manifest;
pub mod parser;
pub mod resolver;
pub mod search;
pub mod types;

/// Crate version, exposed for fmm-store's `write_meta` implementation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
