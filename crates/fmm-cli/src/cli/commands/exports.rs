use anyhow::Result;
use colored::Colorize;
use fmm_core::manifest::{FileEntry, Manifest};

use super::{load_manifest, warn_no_sidecars};

type ExportMatch = (String, String, Option<[usize; 2]>);
type ExportMatcher = Box<dyn Fn(&str) -> bool>;

#[derive(serde::Serialize)]
struct ExportJson {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

pub fn exports(
    pattern: Option<&str>,
    file: Option<&str>,
    directory: Option<&str>,
    limit: Option<usize>,
    offset: usize,
    json_output: bool,
) -> Result<()> {
    if file.is_some() {
        validate_file_args(pattern, directory)?;
    }

    let (_, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if let Some(file_path) = file {
        print_file_exports(&manifest, file_path, json_output)?;
    } else if let Some(pat) = pattern {
        print_pattern_exports(&manifest, pat, directory, limit, offset, json_output)?;
    } else {
        print_all_exports(&manifest, directory, limit, offset, json_output)?;
    }

    Ok(())
}

fn validate_file_args(pattern: Option<&str>, directory: Option<&str>) -> Result<()> {
    if pattern.is_some() {
        anyhow::bail!("--file cannot be combined with a pattern");
    }
    if directory.is_some() {
        anyhow::bail!("--file cannot be combined with --dir");
    }
    Ok(())
}

fn print_file_exports(manifest: &Manifest, file_path: &str, json_output: bool) -> Result<()> {
    let entry = manifest
        .files
        .get(file_path)
        .ok_or_else(|| anyhow::anyhow!("File '{}' not found in manifest", file_path))?;

    if json_output {
        let json = FileExportsJson {
            file: file_path.to_string(),
            exports: entry_exports_json(file_path, entry),
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            fmm_core::format::format_list_exports_file(file_path, entry)
        );
    }

    Ok(())
}

fn print_pattern_exports(
    manifest: &Manifest,
    pattern: &str,
    directory: Option<&str>,
    limit: Option<usize>,
    offset: usize,
    json_output: bool,
) -> Result<()> {
    let matcher = export_matcher(pattern)?;
    let mut matches = collect_pattern_matches(manifest, directory, &*matcher);
    matches.sort_by_key(|a| a.0.to_lowercase());
    let total = matches.len();
    let (page_start, page_end) = page_bounds(total, limit, offset);
    let matches = &matches[page_start..page_end];

    if json_output {
        let json: Vec<ExportJson> = matches
            .iter()
            .map(|(name, file, lines)| ExportJson {
                name: name.clone(),
                file: file.clone(),
                lines: *lines,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else if total == 0 {
        println!("{} No exports matching '{}'", "!".yellow(), pattern);
        println!(
            "\n  {} Export search is case-insensitive. Try a shorter term.",
            "hint:".cyan()
        );
    } else {
        println!(
            "{}",
            fmm_core::format::format_list_exports_pattern(matches, total, page_start)
        );
    }

    Ok(())
}

fn export_matcher(pattern: &str) -> Result<ExportMatcher> {
    const METACHAR: &[char] = &['^', '$', '[', '(', '\\', '.', '*', '+', '?', '{'];
    let uses_regex = pattern.chars().any(|c| METACHAR.contains(&c));

    if uses_regex {
        let re = regex::Regex::new(pattern).map_err(|e| anyhow::anyhow!("Invalid pattern: {e}"))?;
        Ok(Box::new(move |name: &str| re.is_match(name)))
    } else {
        let pattern_lower = pattern.to_lowercase();
        Ok(Box::new(move |name: &str| {
            name.to_lowercase().contains(&pattern_lower)
        }))
    }
}

fn collect_pattern_matches(
    manifest: &Manifest,
    directory: Option<&str>,
    matcher: &dyn Fn(&str) -> bool,
) -> Vec<ExportMatch> {
    let mut matches: Vec<ExportMatch> = manifest
        .export_index
        .iter()
        .filter(|(name, path)| path_matches_directory(path, directory) && matcher(name))
        .map(|(name, path)| {
            let lines = manifest
                .export_locations
                .get(name)
                .and_then(|loc| loc.lines.as_ref())
                .map(|l| [l.start, l.end]);
            (name.clone(), path.clone(), lines)
        })
        .collect();

    for (dotted_name, loc) in &manifest.method_index {
        if matcher(dotted_name) && path_matches_directory(&loc.file, directory) {
            let lines = loc.lines.as_ref().map(|l| [l.start, l.end]);
            matches.push((dotted_name.clone(), loc.file.clone(), lines));
        }
    }

    matches
}

fn print_all_exports(
    manifest: &Manifest,
    directory: Option<&str>,
    limit: Option<usize>,
    offset: usize,
    json_output: bool,
) -> Result<()> {
    let mut by_file: Vec<(&str, &FileEntry)> = manifest
        .files
        .iter()
        .filter(|(path, entry)| {
            path_matches_directory(path, directory) && !entry.exports.is_empty()
        })
        .map(|(path, entry)| (path.as_str(), entry))
        .collect();
    by_file.sort_by_key(|(path, _)| path.to_lowercase());
    let total = by_file.len();
    let (page_start, page_end) = page_bounds(total, limit, offset);
    let by_file = &by_file[page_start..page_end];

    if json_output {
        let json: Vec<FileExportsJson> = by_file
            .iter()
            .map(|(file, entry)| FileExportsJson {
                file: file.to_string(),
                exports: entry_exports_json(file, entry),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            fmm_core::format::format_list_exports_all(by_file, total, page_start)
        );
    }

    Ok(())
}

fn path_matches_directory(path: &str, directory: Option<&str>) -> bool {
    directory.is_none_or(|dir| path.starts_with(dir))
}

fn page_bounds(total: usize, limit: Option<usize>, offset: usize) -> (usize, usize) {
    let page_start = offset.min(total);
    let page_end = limit.map(|l| (page_start + l).min(total)).unwrap_or(total);
    (page_start, page_end)
}

#[derive(serde::Serialize)]
struct FileExportsJson {
    file: String,
    exports: Vec<ExportJson>,
}

fn entry_exports_json(file: &str, entry: &FileEntry) -> Vec<ExportJson> {
    entry
        .exports
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let lines = entry
                .export_lines
                .as_ref()
                .and_then(|el| el.get(i))
                .filter(|l| l.start > 0)
                .map(|l| [l.start, l.end]);
            ExportJson {
                name: name.clone(),
                file: file.to_string(),
                lines,
            }
        })
        .collect()
}
