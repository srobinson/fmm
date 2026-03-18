//! `fmm_search` tool implementation.

use crate::manifest::Manifest;
use crate::mcp::args::SearchArgs;
use serde_json::Value;

pub(in crate::mcp) fn tool_search(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: SearchArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let has_filters = args.export.is_some()
        || args.imports.is_some()
        || args.depends_on.is_some()
        || args.min_loc.is_some()
        || args.max_loc.is_some();

    let term = args.term;
    let limit = args.limit;
    let filters = crate::search::SearchFilters {
        export: args.export,
        imports: args.imports,
        depends_on: args.depends_on,
        min_loc: args.min_loc,
        max_loc: args.max_loc,
    };

    if let Some(term) = term {
        let mut result = crate::search::bare_search(manifest, &term, limit);
        // When structured filters are also present, intersect with AND semantics:
        // keep only exports/files/imports that are in the filter file set.
        if has_filters {
            let filter_results = crate::search::filter_search(manifest, &filters);
            let filter_files: std::collections::HashSet<&str> =
                filter_results.iter().map(|r| r.file.as_str()).collect();
            result
                .exports
                .retain(|h| filter_files.contains(h.file.as_str()));
            result.files.retain(|f| filter_files.contains(f.as_str()));
            result.imports.iter_mut().for_each(|h| {
                h.files.retain(|f| filter_files.contains(f.as_str()));
            });
            result.imports.retain(|h| !h.files.is_empty());
            result.named_import_hits.iter_mut().for_each(|h| {
                h.files.retain(|f| filter_files.contains(f.as_str()));
            });
            result.named_import_hits.retain(|h| !h.files.is_empty());
            // Stale truncation count is meaningless after filter intersection —
            // exports were dropped because no matching files export them, not
            // because the relevance cap was hit. Clear it to avoid a misleading
            // "[N fuzzy matches — showing top 0]" notice.
            result.total_exports = None;
        }
        let mut formatted = crate::format::format_bare_search(&result, false);
        if has_filters && result.exports.is_empty() && !result.files.is_empty() {
            formatted.push_str(&format!(
                "\n[No exports matching '{}' found in the {} matching file{}]",
                term,
                result.files.len(),
                if result.files.len() == 1 { "" } else { "s" }
            ));
        }
        return Ok(formatted);
    }

    // Structured filter search (no term)
    let results = crate::search::filter_search(manifest, &filters);
    let total_count = results.len();
    let formatted = crate::format::format_filter_search(&results, false);

    // ALP-861: when depends_on is used, add a count header and transitive/direct clarification.
    if let Some(ref dep_path) = filters.depends_on {
        let limit_note = if total_count > 0 {
            format!(
                "{} file{} depend on {} (transitive — includes indirect dependents).",
                total_count,
                if total_count == 1 { "" } else { "s" },
                dep_path,
            )
        } else {
            format!("0 files depend on {} (transitive).", dep_path)
        };
        let footer = format!(
            "# For direct dependents only: fmm_dependency_graph({})",
            dep_path
        );
        return Ok(format!(
            "# {}

{}

{}",
            limit_note, formatted, footer
        ));
    }

    Ok(formatted)
}
