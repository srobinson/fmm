use fmm_core::dupes::{DupeOptions, find_dupe_clusters};
use fmm_core::manifest::Manifest;
use serde_json::Value;

use crate::mcp::args::DupeClustersArgs;

pub(in crate::mcp) fn tool_dupe_clusters(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: DupeClustersArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;
    let opts = DupeOptions::from_args(
        args.directory,
        args.kind.unwrap_or_default(),
        args.min_score,
        args.limit,
        args.include_tests.unwrap_or(false),
    );
    let result = find_dupe_clusters(manifest, &opts);
    serde_json::to_string_pretty(&result).map_err(|e| format!("Failed to serialize result: {e}"))
}
