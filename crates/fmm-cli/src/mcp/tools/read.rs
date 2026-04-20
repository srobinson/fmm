//! `fmm_read_symbol` tool implementation.

use crate::mcp::args::ReadSymbolArgs;
use crate::read_symbol::{ReadSymbolGuidance, read_symbol_result};
use fmm_core::manifest::Manifest;
use serde_json::Value;

pub(in crate::mcp) fn tool_read_symbol(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: ReadSymbolArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let result = read_symbol_result(
        manifest,
        root,
        &args.name,
        args.truncate.unwrap_or(true),
        ReadSymbolGuidance::Mcp,
    )?;
    Ok(result.format_text(args.line_numbers.unwrap_or(false)))
}
