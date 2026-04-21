//! `fmm_lookup_export` tool implementation.

use crate::mcp::args::LookupExportArgs;
use fmm_core::manifest::Manifest;
use serde_json::Value;

use super::common::missing_file_diagnostic;

pub(in crate::mcp) fn tool_lookup_export(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: LookupExportArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    // Try export_locations first, then export_index for backward compat,
    // then method_index for dotted names like "ClassName.method".
    let (file, symbol_lines) = if let Some(loc) = manifest.export_locations.get(&args.name) {
        (loc.file.clone(), loc.lines.clone())
    } else if let Some(file_path) = manifest.export_index.get(&args.name) {
        (file_path.clone(), None)
    } else if let Some(loc) = manifest.method_index.get(&args.name) {
        (loc.file.clone(), loc.lines.clone())
    } else {
        return Err(format!("Export '{}' not found", args.name));
    };

    let entry = manifest
        .files
        .get(&file)
        .ok_or_else(|| missing_file_diagnostic(root, &file))?;

    // Check export_all for additional definitions (collision detection).
    let collision_note = if let Some(all) = manifest.export_all.get(&args.name) {
        let others: Vec<&str> = all
            .iter()
            .map(|loc| loc.file.as_str())
            .filter(|f| *f != file.as_str())
            .collect();
        if others.is_empty() {
            None
        } else {
            let file_list = others.join(", ");
            Some(format!(
                "⚠ {} additional definition(s) found: [{}] — use fmm_glossary for full collision analysis",
                others.len(),
                file_list
            ))
        }
    } else {
        None
    };

    Ok(fmm_core::format::format_lookup_export(
        &args.name,
        &file,
        symbol_lines.as_ref(),
        entry,
        collision_note.as_deref(),
    ))
}
