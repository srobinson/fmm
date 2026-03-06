use anyhow::Result;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct ListFileJson {
    file: String,
    loc: usize,
    exports: usize,
}

pub fn ls(
    directory: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
    group_by: Option<&str>,
    filter: &str,
    json_output: bool,
) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if !matches!(
        sort_by,
        "name" | "loc" | "exports" | "downstream" | "modified"
    ) {
        anyhow::bail!(
            "Invalid --sort-by '{}'. Valid values: name, loc, exports, downstream, modified.",
            sort_by
        );
    }
    if let Some(o) = order {
        if !matches!(o, "asc" | "desc") {
            anyhow::bail!("Invalid --order '{}'. Valid values: asc, desc.", o);
        }
    }

    // Normalise "." / "./" to None — they should list the full repo root,
    // matching the behaviour of omitting the directory parameter entirely.
    let directory = directory.and_then(|d| {
        if matches!(d, "." | "./") {
            None
        } else {
            Some(d)
        }
    });

    let config = crate::config::Config::load_from_dir(&root).unwrap_or_default();

    let mut entries: Vec<(&str, usize, usize, usize, Option<&str>)> = manifest
        .files
        .iter()
        .filter(|(path, _)| {
            if let Some(d) = directory {
                if !path.starts_with(d) {
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
            let downstream = manifest
                .reverse_deps
                .get(path.as_str())
                .map(|v| v.len())
                .unwrap_or(0);
            let modified = entry.modified.as_deref();
            (
                path.as_str(),
                entry.loc,
                entry.exports.len(),
                downstream,
                modified,
            )
        })
        .collect();

    // Rollup mode: group by immediate subdirectory.
    if group_by == Some("subdir") {
        let stripped: Vec<(&str, usize, usize)> =
            entries.iter().map(|(p, l, e, _, _)| (*p, *l, *e)).collect();
        let total_files = stripped.len();
        let total_loc: usize = stripped.iter().map(|(_, loc, _)| loc).sum();
        let buckets = crate::format::compute_rollup_buckets(&stripped, directory, sort_by, order);
        println!(
            "{}",
            crate::format::format_list_files_rollup(directory, &buckets, total_files, total_loc)
        );
        return Ok(());
    }

    let desc = match sort_by {
        "loc" | "exports" | "downstream" | "modified" => order != Some("asc"),
        _ => order == Some("desc"),
    };

    match sort_by {
        "loc" => {
            if desc {
                entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| a.cmp(b));
            }
        }
        "exports" => {
            if desc {
                entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| a.cmp(b));
            }
        }
        "downstream" => {
            if desc {
                entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| a.cmp(b));
            }
        }
        "modified" => {
            if desc {
                entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| a.cmp(b));
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

    if json_output {
        let json: Vec<ListFileJson> = entries
            .iter()
            .map(|(file, loc, exports, _, _)| ListFileJson {
                file: file.to_string(),
                loc: *loc,
                exports: *exports,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        let show_modified = sort_by == "modified";
        println!(
            "{}",
            crate::format::format_list_files(
                directory,
                &entries,
                total,
                total_loc,
                largest,
                0,
                show_modified,
            )
        );
    }

    Ok(())
}
