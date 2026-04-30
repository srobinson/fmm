//! `fmm_list_files` tool implementation.

use crate::mcp::args::ListFilesArgs;
use fmm_core::manifest::Manifest;
use fmm_core::search::DependencyGraphQuery;
use serde_json::Value;

use super::common::{build_rollup, glob_filename_matches};

type ListEntry<'a> = (&'a str, usize, usize, usize, Option<&'a str>);

pub(in crate::mcp) fn tool_list_files(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    const DEFAULT_LIMIT: usize = 200;

    let args: ListFilesArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    // Normalise "." / "./" to None so callers get the full index, matching
    // the behaviour of omitting the directory parameter entirely.
    let dir = args.directory.as_deref().and_then(|d| {
        if matches!(d, "." | "./") {
            None
        } else {
            Some(d)
        }
    });
    let pat = args.pattern.as_deref();
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);
    let sort_by = args.sort_by.as_deref().unwrap_or("loc");
    let order = args.order.as_deref();
    let group_by = args.group_by.as_deref();
    let filter = args.filter.as_deref().unwrap_or("all");

    if !matches!(
        sort_by,
        "name" | "path" | "loc" | "exports" | "downstream" | "modified"
    ) {
        return Err(format!(
            "Invalid sort_by '{}'. Valid values: name, path, loc, exports, downstream, modified.",
            sort_by
        ));
    }
    let sort_by = if sort_by == "path" { "name" } else { sort_by };
    if let Some(o) = order
        && !matches!(o, "asc" | "desc")
    {
        return Err(format!("Invalid order '{}'. Valid values: asc, desc.", o));
    }
    if let Some(g) = group_by
        && g != "subdir"
    {
        return Err(format!("Invalid group_by '{}'. Valid values: subdir.", g));
    }
    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }

    let mut entries = collect_entries(manifest, root, dir, pat, filter);

    // Rollup mode: group by immediate subdirectory.
    if group_by == Some("subdir") {
        // Rollup only uses (path, loc, exports), so strip downstream/modified.
        let stripped: Vec<(&str, usize, usize)> =
            entries.iter().map(|(p, l, e, _, _)| (*p, *l, *e)).collect();
        return Ok(build_rollup(stripped, dir, sort_by, order));
    }

    // Smart defaults: loc/exports/downstream/modified sort descending; name sorts ascending.
    let desc = match sort_by {
        "loc" | "exports" | "downstream" | "modified" => order != Some("asc"),
        _ => order == Some("desc"),
    };

    match sort_by {
        "loc" => {
            if desc {
                entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| b.cmp(a));
            } else {
                entries.sort_by_key(|(_, a, _, _, _)| *a);
            }
        }
        "exports" => {
            if desc {
                entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| b.cmp(a));
            } else {
                entries.sort_by_key(|(_, _, a, _, _)| *a);
            }
        }
        "downstream" => {
            if desc {
                entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| b.cmp(a));
            } else {
                entries.sort_by_key(|(_, _, _, a, _)| *a);
            }
        }
        "modified" => {
            // Lexicographic sort on YYYY-MM-DD strings works correctly for date ordering.
            // Files with no modified date sort last.
            if desc {
                entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| b.cmp(a));
            } else {
                entries.sort_by_key(|(_, _, _, _, a)| *a);
            }
        }
        _ => {
            if desc {
                entries.sort_by(|(a, _, _, _, _), (b, _, _, _, _)| {
                    b.to_lowercase().cmp(&a.to_lowercase())
                });
            } else {
                entries.sort_by_key(|(path, _, _, _, _)| path.to_lowercase());
            }
        }
    }

    let total = entries.len();
    let total_loc: usize = entries.iter().map(|(_, loc, _, _, _)| loc).sum();
    let largest = entries
        .iter()
        .max_by_key(|(_, loc, _, _, _)| loc)
        .map(|(path, loc, _, _, _)| (*path, *loc));
    let show_modified = sort_by == "modified";
    let page: Vec<(&str, usize, usize, usize, Option<&str>)> =
        entries.into_iter().skip(offset).take(limit).collect();

    Ok(fmm_core::format::format_list_files(
        dir,
        &page,
        total,
        total_loc,
        largest,
        offset,
        show_modified,
    ))
}

fn collect_entries<'a>(
    manifest: &'a Manifest,
    root: &std::path::Path,
    dir: Option<&str>,
    pat: Option<&str>,
    filter: &str,
) -> Vec<ListEntry<'a>> {
    let config = fmm_core::config::Config::load_from_dir(root).unwrap_or_default();
    let graph_query = DependencyGraphQuery::new(manifest).ok();

    manifest
        .files
        .iter()
        .filter(|(path, _)| {
            if let Some(d) = dir
                && !path.starts_with(d)
            {
                return false;
            }
            match filter {
                "tests" if !config.is_test_file(path) => return false,
                "source" if config.is_test_file(path) => return false,
                _ => {}
            }
            if let Some(p) = pat {
                let filename = path.rsplit('/').next().unwrap_or(path.as_str());
                if !glob_filename_matches(p, filename) {
                    return false;
                }
            }
            true
        })
        .map(|(path, entry)| {
            let downstream = graph_query
                .as_ref()
                .map_or(0, |graph| graph.downstream_count(path));
            (
                path.as_str(),
                entry.loc,
                entry.exports.len(),
                downstream,
                entry.modified.as_deref(),
            )
        })
        .collect()
}
