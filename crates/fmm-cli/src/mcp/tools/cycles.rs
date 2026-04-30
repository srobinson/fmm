//! `fmm_dependency_cycles` tool implementation.

use crate::mcp::args::DependencyCyclesArgs;
use fmm_core::manifest::Manifest;
use serde_json::Value;

use super::common::{missing_file_diagnostic, validate_not_directory};

pub(in crate::mcp) fn tool_dependency_cycles(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: DependencyCyclesArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    if let Some(file) = &args.file {
        validate_not_directory(file, root)?;
        if !manifest.files.contains_key(file) {
            return Err(missing_file_diagnostic(root, file));
        }
    }

    let filter = args.filter.as_deref().unwrap_or("all");
    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }

    let edge_mode = crate::cycle_report::parse_edge_mode(args.edge_mode.as_deref())?;
    let config = fmm_core::config::Config::load_from_dir(root).unwrap_or_default();
    let cycles = fmm_core::search::dependency_cycles(manifest, args.file.as_deref(), edge_mode)
        .map_err(|e| e.to_string())?;
    let cycles =
        crate::cycle_report::filter_cycles(cycles, filter, |path| config.is_test_file(path));

    Ok(fmm_core::format::format_dependency_cycles(&cycles))
}
