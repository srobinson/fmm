//! `fmm_glossary` tool implementation.

use crate::mcp::args::GlossaryArgs;
use fmm_core::manifest::Manifest;
use serde_json::Value;

pub(in crate::mcp) fn tool_glossary(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: GlossaryArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let pattern = args.pattern.as_deref().unwrap_or("").trim();
    if pattern.is_empty() {
        return Err(
            "pattern is required — provide a symbol name or substring (e.g. 'run_dispatch', 'config'). \
            A full unfiltered glossary on a large codebase would exceed any useful context window."
                .to_string(),
        );
    }

    let mode = crate::glossary::parse_mode(args.mode.as_deref().unwrap_or("source"));
    let precision =
        crate::glossary::GlossaryPrecision::parse(args.precision.as_deref().unwrap_or("named"))?;
    let result = crate::glossary::compute_glossary(
        manifest,
        root,
        crate::glossary::GlossaryQuery {
            pattern,
            mode,
            limit: args.limit,
            precision,
        },
    )?;

    Ok(crate::glossary::format_text(&result))
}
