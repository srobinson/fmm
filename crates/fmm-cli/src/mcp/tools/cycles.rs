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

    let file_filter =
        crate::cycle_report::CycleFileFilter::parse(args.filter.as_deref().unwrap_or("all"))?;

    let edge_mode = crate::cycle_report::parse_edge_mode(args.edge_mode.as_deref())?;
    let config = fmm_core::config::Config::load_from_dir(root).unwrap_or_default();
    let cycles = fmm_core::search::dependency_cycles_with_path_filter(
        manifest,
        args.file.as_deref(),
        edge_mode,
        |path| file_filter.keeps(path, |candidate| config.is_test_file(candidate)),
    )
    .map_err(|e| e.to_string())?;

    Ok(fmm_core::format::format_dependency_cycles(&cycles))
}
