//! `fmm_find_similar` tool implementation.

use crate::mcp::args::FindSimilarArgs;
use fmm_core::format::format_similar;
use fmm_core::manifest::Manifest;
use fmm_core::similarity::{SimilarOptions, find_similar, probe_for};
use serde_json::Value;

pub(in crate::mcp) fn tool_find_similar(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: FindSimilarArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let probe = probe_for(manifest, &args.name, args.signature, args.kind);
    let opts = SimilarOptions {
        limit: args.limit.unwrap_or(10),
        directory: args.directory,
        include_tests: args.include_tests.unwrap_or(false),
        ..Default::default()
    };

    let matches = find_similar(manifest, &probe, &opts);
    Ok(format_similar(&args.name, &matches))
}
