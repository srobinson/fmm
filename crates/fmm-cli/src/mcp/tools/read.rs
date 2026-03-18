//! `fmm_read_symbol` tool implementation.

use crate::manifest::Manifest;
use crate::mcp::args::ReadSymbolArgs;
use serde_json::Value;

use super::common::{find_concrete_definition, is_reexport_file};

pub(in crate::mcp) fn tool_read_symbol(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: ReadSymbolArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    if args.name.trim().is_empty() {
        return Err(
            "Symbol name must not be empty. Use fmm_list_exports to discover available symbols."
                .to_string(),
        );
    }

    // Colon notation: path/to/file.ts:symbolName — on-demand tree-sitter parse
    // for a non-exported (or any) top-level function in the named file.
    // Checked before the dot notation to avoid ambiguity.
    let (resolved_file, resolved_lines) = if let Some(colon_pos) = args.name.find(':') {
        let file_part = &args.name[..colon_pos];
        let symbol_part = &args.name[colon_pos + 1..];

        // Require the file part to look like a path (contains '/' or '.')
        // so bare symbol names with colons (unlikely) don't accidentally match.
        if (file_part.contains('/') || file_part.contains('.')) && !symbol_part.is_empty() {
            let (start, end) = crate::manifest::private_members::find_top_level_function_range(
                root,
                file_part,
                symbol_part,
            )
            .ok_or_else(|| {
                format!(
                    "Symbol '{}' not found in '{}'. \
                         Note: exported symbols (export function/const) must be read \
                         by plain name — fmm_read_symbol(\"{}\"). \
                         Use fmm_file_outline(file: \"{}\", include_private: true) \
                         to see all top-level declarations.",
                    symbol_part, file_part, symbol_part, file_part
                )
            })?;
            (
                file_part.to_string(),
                Some(crate::manifest::ExportLines { start, end }),
            )
        } else {
            // Doesn't look like a file path — reject rather than silently misrouting.
            // Export names never contain colons; if the user omitted the path separator
            // they should use 'path/to/file.ts:symbol' form.
            return Err(format!(
                "Ambiguous name '{}'. For file:symbol notation, \
                 the file path must contain '/' or '.' (e.g. 'src/helpers.ts:myFn').",
                args.name
            ));
        }
    // Dotted notation: ClassName.method — look up in method_index first.
    // If not found (private method), fall back to on-demand tree-sitter extraction.
    } else if args.name.contains('.') {
        if let Some(loc) = manifest.method_index.get(&args.name) {
            (loc.file.clone(), loc.lines.clone())
        } else {
            // ALP-827: private method fallback — parse the file on demand.
            let dot = args.name.rfind('.').unwrap();
            let class_name = &args.name[..dot];
            let method_name = &args.name[dot + 1..];

            let class_file = manifest
                .export_locations
                .get(class_name)
                .map(|loc| loc.file.clone())
                .ok_or_else(|| {
                    format!(
                        "Method '{}' not found. Class '{}' is not a known export. \
                         Use fmm_file_outline to inspect the file.",
                        args.name, class_name
                    )
                })?;

            let (start, end) = crate::manifest::private_members::find_private_method_range(
                root,
                &class_file,
                class_name,
                method_name,
            )
            .ok_or_else(|| {
                format!(
                    "Method '{}' not found. '{}' is not a public or private method of \
                         '{}'. Use fmm_file_outline(include_private: true) to see all members.",
                    args.name, method_name, class_name
                )
            })?;

            (
                class_file,
                Some(crate::manifest::ExportLines { start, end }),
            )
        }
    } else {
        let location = manifest
            .export_locations
            .get(&args.name)
            .ok_or_else(|| format!("Export '{}' not found. Use fmm_list_exports or fmm_search to discover available symbols.", args.name))?;

        // If the winning location is a re-export hub (index file), try to find the
        // concrete definition in a nearby non-index file that also exports this symbol.
        if is_reexport_file(&location.file) {
            if let Some((concrete_file, concrete_lines)) =
                find_concrete_definition(manifest, &args.name, &location.file)
            {
                (concrete_file, Some(concrete_lines))
            } else {
                (location.file.clone(), location.lines.clone())
            }
        } else {
            (location.file.clone(), location.lines.clone())
        }
    };

    let lines = resolved_lines.ok_or_else(|| {
        format!(
            "No line range for '{}' in '{}' — run 'fmm generate' to re-index",
            args.name, resolved_file,
        )
    })?;

    let source_path = root.join(&resolved_file);
    let content = std::fs::read_to_string(&source_path)
        .map_err(|e| format!("Cannot read '{}': {}", resolved_file, e))?;

    let source_lines: Vec<&str> = content.lines().collect();
    let start = lines.start.saturating_sub(1);
    let end = lines.end.min(source_lines.len());

    if start >= source_lines.len() {
        return Err(format!(
            "Line range [{}, {}] out of bounds for '{}' ({} lines)",
            lines.start,
            lines.end,
            resolved_file,
            source_lines.len()
        ));
    }

    let symbol_source = source_lines[start..end].join("\n");

    // Bare class redirect: when a bare class name (no dot, no colon) would exceed
    // the 10KB cap and truncate was not explicitly disabled, return an outline with
    // redirect hints instead of a misleading partial view of the class body.
    let is_bare_name = !args.name.contains('.') && !args.name.contains(':');
    let should_truncate = args.truncate.unwrap_or(true);
    if is_bare_name
        && should_truncate
        && symbol_source.len() > crate::mcp::McpServer::MAX_RESPONSE_BYTES
    {
        // Check if this class has methods registered in the file entry.
        if let Some(file_entry) = manifest.files.get(&resolved_file) {
            let prefix = format!("{}.", args.name);
            let mut class_methods: Vec<(&str, &crate::manifest::ExportLines)> = file_entry
                .methods
                .as_ref()
                .map(|m| {
                    m.iter()
                        .filter(|(k, _)| k.starts_with(&prefix))
                        .map(|(k, v)| (k.trim_start_matches(&prefix), v))
                        .collect()
                })
                .unwrap_or_default();
            if !class_methods.is_empty() {
                // Sort by line start order for readability.
                class_methods.sort_by_key(|(_, el)| el.start);
                return Ok(crate::format::format_class_redirect(
                    &args.name,
                    &resolved_file,
                    &lines,
                    &class_methods,
                ));
            }
        }
    }

    Ok(crate::format::format_read_symbol(
        &args.name,
        &resolved_file,
        &lines,
        &symbol_source,
        args.line_numbers.unwrap_or(false),
    ))
}
