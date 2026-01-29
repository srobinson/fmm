//! Plugin loading for external parsers.
//!
//! This module will eventually support dynamic library loading from ~/.fmm/plugins/.
//! For now, it provides the API surface that external plugins will implement.
//!
//! See docs/plugin-architecture.md for the full design.

use crate::parser::ParserRegistry;
use anyhow::Result;

/// Scan the plugin directory and register any discovered parsers.
/// Currently a no-op stub — external plugins are not yet supported.
#[allow(dead_code)]
pub fn load_plugins(_registry: &mut ParserRegistry) -> Result<()> {
    // Future: scan ~/.fmm/plugins/ for .dylib/.so/.dll files,
    // validate metadata, and register their parser factories.
    Ok(())
}
