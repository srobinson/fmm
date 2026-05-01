use anyhow::Result;
use fmm_core::search::DependencyGraphQuery;

use super::{load_manifest, warn_no_sidecars};

type ListEntry<'a> = (&'a str, usize, usize, usize, Option<&'a str>);

#[derive(serde::Serialize)]
struct ListFileJson {
    file: String,
    loc: usize,
    exports: usize,
    downstream: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn ls(
    directory: Option<&str>,
    pattern: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
    group_by: Option<&str>,
    filter: &str,
    limit: Option<usize>,
    offset: usize,
    json_output: bool,
) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if !matches!(
        sort_by,
        "name" | "path" | "loc" | "exports" | "downstream" | "modified"
    ) {
        anyhow::bail!(
            "Invalid --sort-by '{}'. Valid values: name, path, loc, exports, downstream, modified.",
            sort_by
        );
    }
    let sort_by = if sort_by == "path" { "name" } else { sort_by };
    if let Some(o) = order
        && !matches!(o, "asc" | "desc")
    {
        anyhow::bail!("Invalid --order '{}'. Valid values: asc, desc.", o);
    }

    // Normalise "." / "./" to None so they list the full repo root,
    // matching the behaviour of omitting the directory parameter entirely.
    let directory = directory.and_then(|d| {
        if matches!(d, "." | "./") {
            None
        } else {
            Some(d)
        }
    });

    let config = fmm_core::config::Config::load_from_dir(&root).unwrap_or_default();

    let glob_pattern = pattern
        .map(|p| {
            glob::Pattern::new(p)
                .map_err(|e| anyhow::anyhow!("Invalid --pattern glob '{}': {}", p, e))
        })
        .transpose()?;

    let mut entries = collect_entries(&manifest, directory, glob_pattern.as_ref(), filter, &config);

    // Rollup mode: group by immediate subdirectory.
    if group_by == Some("subdir") {
        let stripped: Vec<(&str, usize, usize)> =
            entries.iter().map(|(p, l, e, _, _)| (*p, *l, *e)).collect();
        let total_files = stripped.len();
        let total_loc: usize = stripped.iter().map(|(_, loc, _)| loc).sum();
        let buckets =
            fmm_core::format::compute_rollup_buckets(&stripped, directory, sort_by, order);
        println!(
            "{}",
            fmm_core::format::format_list_files_rollup(directory, &buckets, total_files, total_loc)
        );
        return Ok(());
    }

    sort_entries(&mut entries, sort_by, order);

    let total = entries.len();
    let total_loc: usize = entries.iter().map(|(_, loc, _, _, _)| loc).sum();
    let largest = entries
        .iter()
        .max_by_key(|(_, loc, _, _, _)| loc)
        .map(|(path, loc, _, _, _)| (*path, *loc));

    // Apply pagination.
    let page_start = offset.min(total);
    let page_end = limit.map(|l| (page_start + l).min(total)).unwrap_or(total);
    let entries = &entries[page_start..page_end];

    if json_output {
        let json: Vec<ListFileJson> = entries
            .iter()
            .map(|(file, loc, exports, downstream, _)| ListFileJson {
                file: file.to_string(),
                loc: *loc,
                exports: *exports,
                downstream: *downstream,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        let show_modified = sort_by == "modified";
        println!(
            "{}",
            fmm_core::format::format_list_files(
                directory,
                entries,
                total,
                total_loc,
                largest,
                page_start,
                show_modified,
            )
        );
    }

    Ok(())
}

fn collect_entries<'a>(
    manifest: &'a fmm_core::manifest::Manifest,
    directory: Option<&str>,
    glob_pattern: Option<&glob::Pattern>,
    filter: &str,
    config: &fmm_core::config::Config,
) -> Vec<ListEntry<'a>> {
    let graph_query = DependencyGraphQuery::new(manifest).ok();

    manifest
        .files
        .iter()
        .filter(|(path, _)| {
            if let Some(d) = directory
                && !path.starts_with(d)
            {
                return false;
            }
            if let Some(pat) = glob_pattern {
                let filename = std::path::Path::new(path.as_str())
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !pat.matches(filename) {
                    return false;
                }
            }
            match filter {
                "tests" => config.is_test_file(path),
                "source" => !config.is_test_file(path),
                _ => true,
            }
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

fn sort_entries(entries: &mut [ListEntry<'_>], sort_by: &str, order: Option<&str>) {
    let desc = match sort_by {
        "loc" | "exports" | "downstream" | "modified" => order != Some("asc"),
        _ => order == Some("desc"),
    };

    match sort_by {
        "loc" if desc => entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| b.cmp(a)),
        "loc" => entries.sort_by_key(|(_, loc, _, _, _)| *loc),
        "exports" if desc => entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| b.cmp(a)),
        "exports" => entries.sort_by_key(|(_, _, exports, _, _)| *exports),
        "downstream" if desc => entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| b.cmp(a)),
        "downstream" => entries.sort_by_key(|(_, _, _, downstream, _)| *downstream),
        "modified" if desc => entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| b.cmp(a)),
        "modified" => entries.sort_by_key(|(_, _, _, _, modified)| *modified),
        _ if desc => {
            entries.sort_by_key(|(path, _, _, _, _)| std::cmp::Reverse(path.to_lowercase()))
        }
        _ => entries.sort_by_key(|(path, _, _, _, _)| path.to_lowercase()),
    }
}
