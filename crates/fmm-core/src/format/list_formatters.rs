//! List exports and list files formatters.

use crate::format::yaml_escape;
use crate::manifest::FileEntry;

use super::helpers::push_exports_map;

/// Format list exports for a pattern search: column-aligned text with optional pagination.
///
/// - `matches`: the current page of results (already sliced by offset/limit)
/// - `total`: total number of matches before pagination
/// - `offset`: the page start index (0-based)
pub fn format_list_exports_pattern(
    matches: &[(String, String, Option<[usize; 2]>)],
    total: usize,
    offset: usize,
) -> String {
    if matches.is_empty() {
        return String::new();
    }
    let name_width = matches.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    let file_width = matches.iter().map(|(_, f, _)| f.len()).max().unwrap_or(0);

    let mut out = Vec::new();
    let showing = matches.len();
    if showing < total {
        let end = offset + showing;
        out.push(format!("# showing: {}-{} of {}", offset + 1, end, total));
        if end < total {
            out.push(format!("# next: Use offset={} to continue.", end));
        }
    }
    for (name, file, lines) in matches {
        let lines_str = match lines {
            Some([s, e]) => format!("  [{}, {}]", s, e),
            None => String::new(),
        };
        out.push(format!(
            "{:<nw$}  {:<fw$}{}",
            name,
            file,
            lines_str,
            nw = name_width,
            fw = file_width,
        ));
    }
    out.join("\n")
}

/// Format list exports for a specific file: sidecar YAML.
pub fn format_list_exports_file(file: &str, entry: &FileEntry) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("file: {}", yaml_escape(file)));
    push_exports_map(&mut lines, &entry.exports, entry.export_lines.as_ref());
    lines.join("\n")
}

/// Format list exports for all files: multi-document sidecar YAML with optional pagination.
///
/// - `files`: the current page of entries (already sliced by offset/limit)
/// - `total`: total number of matching files before pagination
/// - `offset`: the page start index (0-based)
pub fn format_list_exports_all(
    files: &[(&str, &FileEntry)],
    total: usize,
    offset: usize,
) -> String {
    let mut docs = Vec::new();
    let showing = files.len();
    if showing > 0 && showing < total {
        let end = offset + showing;
        let mut header = Vec::new();
        header.push("---".to_string());
        header.push(format!("showing: {}-{} of {}", offset + 1, end, total));
        if end < total {
            header.push(format!("next: Use offset={} to continue.", end));
        }
        docs.push(header.join("\n"));
    }
    for (file, entry) in files {
        let mut lines = Vec::new();
        lines.push("---".to_string());
        lines.push(format!("file: {}", yaml_escape(file)));

        if entry.exports.is_empty() {
            lines.push("exports: []".to_string());
        } else {
            let items: Vec<String> = entry.exports.iter().map(|s| yaml_escape(s)).collect();
            lines.push(format!("exports: [{}]", items.join(", ")));
        }
        docs.push(lines.join("\n"));
    }
    docs.join("\n")
}

/// Format list files result as compact YAML with optional pagination metadata.
///
/// Entry tuple: `(path, loc, exports, downstream_count, modified)`.
///
/// - `directory`: directory prefix filter, shown in header
/// - `files`: the current page of entries (already sliced by offset/limit)
/// - `total`: total number of matching files before pagination
/// - `total_loc`: sum of LOC across all matching files (full set, not page)
/// - `largest`: path and LOC of the largest file in the full set
/// - `offset`: the page start index (0-based)
/// - `show_modified`: when true, include the modified date in each file row
pub fn format_list_files(
    directory: Option<&str>,
    files: &[(&str, usize, usize, usize, Option<&str>)],
    total: usize,
    total_loc: usize,
    largest: Option<(&str, usize)>,
    offset: usize,
    show_modified: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if let Some(dir) = directory {
        lines.push(format!("directory: {}", yaml_escape(dir)));
    }
    // Summary: file count, total LOC, largest file — scoped to the filtered set
    let summary = match largest {
        Some((path, loc)) => format!(
            "{} files · {} LOC · largest: {} ({} LOC)",
            format_count(total),
            format_count(total_loc),
            path,
            format_count(loc),
        ),
        None => format!(
            "{} files · {} LOC",
            format_count(total),
            format_count(total_loc)
        ),
    };
    lines.push(format!("summary: {}", summary));
    lines.push(format!("total: {}", total));
    let showing = files.len();
    if showing > 0 && showing < total {
        let end = offset + showing;
        lines.push(format!("showing: {}-{} of {}", offset + 1, end, total,));
        if end < total {
            lines.push(format!("next: Use offset={} to continue.", end));
        }
    }
    if !files.is_empty() {
        lines.push("files:".to_string());
        // Column width for alignment
        let path_width = files
            .iter()
            .map(|(p, _, _, _, _)| p.len())
            .max()
            .unwrap_or(0);
        for (path, loc, exports, downstream, modified) in files {
            let downstream_str = if *downstream > 0 {
                format!(", ↓ {} downstream", downstream)
            } else {
                String::new()
            };
            let modified_str = if show_modified {
                match modified {
                    Some(d) => format!(", modified: {}", d),
                    None => String::new(),
                }
            } else {
                String::new()
            };
            lines.push(format!(
                "  - {:<pw$}  # loc: {}, exports: {}{}{}",
                path,
                loc,
                exports,
                downstream_str,
                modified_str,
                pw = path_width,
            ));
        }
    }
    // ALP-860: disclose that downstream count is local-only (cross-package importers excluded).
    if !files.is_empty() {
        lines.push(
            "# ↓ N = local relative-import dependents only. Cross-package importers not included."
                .to_string(),
        );
    }

    lines.join("\n")
}

/// Compute directory-rollup buckets from a flat file list.
///
/// Groups entries by their immediate subdirectory relative to `prefix`,
/// sums file counts and LOC, and returns sorted `(dir, file_count, total_loc)` triples.
/// `sort_by` can be `"loc"` (default desc) or `"name"` (default asc); `order` overrides.
pub fn compute_rollup_buckets(
    entries: &[(&str, usize, usize)],
    prefix: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
) -> Vec<(String, usize, usize)> {
    use std::collections::HashMap;
    let prefix = prefix.unwrap_or("");
    // Normalise to include trailing slash so strip_prefix removes the full
    // directory segment.  "packages" → "packages/", "" stays "".
    // Without this, strip_prefix("packages") on "packages/core/foo.ts" returns
    // "/core/foo.ts" and the leading '/' makes the first split segment empty,
    // collapsing every file into a single "packages/" bucket.
    let prefix_dir: String = if prefix.is_empty() {
        String::new()
    } else if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{}/", prefix)
    };
    let mut buckets: HashMap<String, (usize, usize)> = HashMap::new();

    for (path, loc, _) in entries {
        let rel = path.strip_prefix(&prefix_dir).unwrap_or(path);
        let bucket = if let Some(idx) = rel.find('/') {
            format!("{}{}/", prefix_dir, &rel[..idx])
        } else if prefix_dir.is_empty() {
            "(root)".to_string()
        } else {
            prefix_dir.clone()
        };
        let e = buckets.entry(bucket).or_insert((0, 0));
        e.0 += 1;
        e.1 += loc;
    }

    let mut bucket_vec: Vec<(String, usize, usize)> = buckets
        .into_iter()
        .map(|(dir, (count, loc))| (dir, count, loc))
        .collect();

    let desc = match sort_by {
        "name" => order == Some("desc"),
        _ => order != Some("asc"),
    };

    match sort_by {
        "name" => {
            if desc {
                bucket_vec.sort_by(|(a, _, _), (b, _, _)| b.cmp(a));
            } else {
                bucket_vec.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
            }
        }
        _ => {
            if desc {
                bucket_vec.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));
            } else {
                bucket_vec.sort_by_key(|(_, _, a)| *a);
            }
        }
    }

    bucket_vec
}

/// Format list files in directory-rollup mode (group_by="subdir").
///
/// Each bucket row shows: directory path, file count, total LOC.
/// `buckets` is a sorted slice of `(dir_path, file_count, total_loc)`.
pub fn format_list_files_rollup(
    directory: Option<&str>,
    buckets: &[(String, usize, usize)],
    total_files: usize,
    total_loc: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    if let Some(dir) = directory {
        lines.push(format!("directory: {}", yaml_escape(dir)));
    }
    lines.push(format!(
        "summary: {} files · {} LOC",
        format_count(total_files),
        format_count(total_loc),
    ));
    lines.push(format!("buckets: {}", buckets.len()));

    if !buckets.is_empty() {
        let dir_width = buckets.iter().map(|(d, _, _)| d.len()).max().unwrap_or(0);
        let count_width = buckets
            .iter()
            .map(|(_, n, _)| format_count(*n).len())
            .max()
            .unwrap_or(0);
        for (dir, count, loc) in buckets {
            lines.push(format!(
                "  {:<dw$}  {:>cw$} files  · {} LOC",
                dir,
                format_count(*count),
                format_count(*loc),
                dw = dir_width,
                cw = count_width,
            ));
        }
    }
    lines.join("\n")
}

/// Format a number with comma thousands separators (e.g. 1234567 → "1,234,567").
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::FileEntry;

    #[test]
    fn format_count_inserts_commas() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
        assert_eq!(format_count(487341), "487,341");
    }

    #[test]
    fn list_files_summary_header_included() {
        // Two files: alpha (100 LOC, 2 exports, 5 downstream) and beta (30 LOC, 1 export, 0 downstream)
        let files = vec![
            ("src/alpha.ts", 100usize, 2usize, 5usize, None),
            ("src/beta.ts", 30, 1, 0, None),
        ];
        let out = format_list_files(None, &files, 2, 130, Some(("src/alpha.ts", 100)), 0, false);
        assert!(
            out.contains("summary:"),
            "summary line should appear; got:\n{}",
            out
        );
        assert!(
            out.contains("2 files"),
            "summary should show file count; got:\n{}",
            out
        );
        assert!(
            out.contains("130 LOC"),
            "summary should show total LOC; got:\n{}",
            out
        );
        assert!(
            out.contains("largest: src/alpha.ts (100 LOC)"),
            "summary should show largest file; got:\n{}",
            out
        );
        // Row format: downstream shown when > 0
        assert!(out.contains("src/alpha.ts"));
        assert!(out.contains("# loc: 100"));
        assert!(out.contains("↓ 5 downstream")); // alpha has 5 downstream
        assert!(!out.contains("↓ 0")); // beta has 0 downstream — not shown
    }

    // Suppress unused import warning — FileEntry is needed for format_list_exports_file
    #[allow(dead_code)]
    fn _uses_file_entry(_: &FileEntry) {}
}
