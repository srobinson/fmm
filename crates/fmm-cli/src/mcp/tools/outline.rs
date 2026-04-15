//! `fmm_file_outline` tool implementation.

use crate::mcp::args::FileOutlineArgs;
use fmm_core::manifest::Manifest;
use serde_json::Value;

use super::common::validate_not_directory;

pub(in crate::mcp) fn tool_file_outline(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: FileOutlineArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    validate_not_directory(&args.file, root)?;

    let entry = manifest.files.get(&args.file).ok_or_else(|| {
        format!(
            "File '{}' not found in manifest. Run 'fmm generate' to index the file.",
            args.file
        )
    })?;

    let include_private = args.include_private.unwrap_or(false);
    let private_by_class = if include_private {
        let class_names: Vec<&str> = entry.exports.iter().map(|s| s.as_str()).collect();
        Some(
            fmm_core::manifest::private_members::extract_private_members(
                root,
                &args.file,
                &class_names,
            ),
        )
    } else {
        None
    };
    let top_level_fns = if include_private {
        let export_names: Vec<&str> = entry.exports.iter().map(|s| s.as_str()).collect();
        let fns = fmm_core::manifest::private_members::extract_top_level_functions(
            root,
            &args.file,
            &export_names,
        );
        Some(fns)
    } else {
        None
    };

    let reexports = manifest.reexports_in_file(&args.file);

    Ok(fmm_core::format::format_file_outline(
        &args.file,
        entry,
        &reexports,
        private_by_class.as_ref(),
        top_level_fns.as_deref(),
    ))
}
