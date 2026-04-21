//! `fmm_dependency_graph` tool implementation.

use crate::mcp::args::DependencyGraphArgs;
use fmm_core::manifest::Manifest;
use serde_json::Value;

use super::common::{missing_file_diagnostic, validate_not_directory};

pub(in crate::mcp) fn tool_dependency_graph(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: DependencyGraphArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    validate_not_directory(&args.file, root)?;

    let entry = manifest
        .files
        .get(&args.file)
        .ok_or_else(|| missing_file_diagnostic(root, &args.file))?;

    let depth = args.depth.unwrap_or(1);
    let filter = args.filter.as_deref().unwrap_or("all");

    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }

    // Build a predicate that determines whether a file path is kept.
    // Loads config once — same heuristic as fmm_list_files filter.
    let config = fmm_core::config::Config::load_from_dir(root).unwrap_or_default();
    let keep = |path: &str| -> bool {
        match filter {
            "source" => !config.is_test_file(path),
            "tests" => config.is_test_file(path),
            _ => true,
        }
    };

    if depth == 1 {
        // depth=1: use existing single-hop implementation for backward compatibility
        let (local, external, downstream) =
            fmm_core::search::dependency_graph(manifest, &args.file, entry);
        let local: Vec<String> = local.into_iter().filter(|p| keep(p)).collect();
        let downstream: Vec<&String> = downstream
            .into_iter()
            .filter(|p| keep(p.as_str()))
            .collect();
        Ok(fmm_core::format::format_dependency_graph(
            &args.file,
            entry,
            &local,
            &external,
            &downstream,
        ))
    } else {
        // depth>1 or depth=-1: BFS transitive traversal with depth annotations
        let (upstream, external, downstream) =
            fmm_core::search::dependency_graph_transitive(manifest, &args.file, entry, depth);
        let upstream: Vec<(String, i32)> = upstream.into_iter().filter(|(p, _)| keep(p)).collect();
        let downstream: Vec<(String, i32)> =
            downstream.into_iter().filter(|(p, _)| keep(p)).collect();
        Ok(fmm_core::format::format_dependency_graph_transitive(
            &args.file,
            entry,
            &upstream,
            &external,
            &downstream,
            depth,
        ))
    }
}
