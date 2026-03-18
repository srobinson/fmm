pub mod parser;

/// Crate version, exposed for fmm-store's `write_meta` implementation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
