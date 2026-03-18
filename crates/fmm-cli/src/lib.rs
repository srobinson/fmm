pub mod cli;
pub mod fs_utils;
pub mod manifest_ext;
pub mod mcp;

// Re-export fmm-core modules so internal `crate::*` references
// continue to work without changing every import site. Consumers gradually
// migrate to `fmm_core::*` as the codebase stabilizes.
pub use fmm_core::config;
pub use fmm_core::extractor;
pub use fmm_core::format;
pub use fmm_core::manifest;
pub use fmm_core::parser;
pub use fmm_core::resolver;
pub use fmm_core::search;
pub use fmm_core::store;
