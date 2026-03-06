use anyhow::Result;
use colored::Colorize;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct ExportJson {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

pub fn exports(
    pattern: Option<&str>,
    directory: Option<&str>,
    limit: Option<usize>,
    offset: usize,
    json_output: bool,
) -> Result<()> {
    let (_, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if let Some(pat) = pattern {
        // Auto-detect regex: metacharacters trigger compiled regex (case-sensitive).
        // Plain patterns keep the existing case-insensitive substring match.
        const METACHAR: &[char] = &['^', '$', '[', '(', '\\', '.', '*', '+', '?', '{'];
        let uses_regex = pat.chars().any(|c| METACHAR.contains(&c));
        let matcher: Box<dyn Fn(&str) -> bool> = if uses_regex {
            let re = regex::Regex::new(pat).map_err(|e| anyhow::anyhow!("Invalid pattern: {e}"))?;
            Box::new(move |name: &str| re.is_match(name))
        } else {
            let pat_lower = pat.to_lowercase();
            Box::new(move |name: &str| name.to_lowercase().contains(&pat_lower))
        };

        let mut matches: Vec<(String, String, Option<[usize; 2]>)> = manifest
            .export_index
            .iter()
            .filter(|(name, path)| {
                if let Some(d) = directory {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                matcher(name)
            })
            .map(|(name, path)| {
                let lines = manifest
                    .export_locations
                    .get(name)
                    .and_then(|loc| loc.lines.as_ref())
                    .map(|l| [l.start, l.end]);
                (name.clone(), path.clone(), lines)
            })
            .collect();

        // Include method_index matches (dotted names like "ClassName.method").
        for (dotted_name, loc) in &manifest.method_index {
            if !matcher(dotted_name) {
                continue;
            }
            if let Some(d) = directory {
                if !loc.file.starts_with(d) {
                    continue;
                }
            }
            let lines = loc.lines.as_ref().map(|l| [l.start, l.end]);
            matches.push((dotted_name.clone(), loc.file.clone(), lines));
        }

        matches.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let total = matches.len();

        // Apply pagination.
        let page_start = offset.min(total);
        let page_end = limit.map(|l| (page_start + l).min(total)).unwrap_or(total);
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
            println!("{} No exports matching '{}'", "!".yellow(), pat);
            println!(
                "\n  {} Export search is case-insensitive. Try a shorter term.",
                "hint:".cyan()
            );
        } else {
            println!(
                "{}",
                crate::format::format_list_exports_pattern(matches, total, page_start)
            );
        }
    } else {
        // No pattern: list all exports grouped by file
        let mut by_file: Vec<(&str, &crate::manifest::FileEntry)> = manifest
            .files
            .iter()
            .filter(|(path, entry)| {
                if let Some(d) = directory {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                !entry.exports.is_empty()
            })
            .map(|(path, entry)| (path.as_str(), entry))
            .collect();
        by_file.sort_by_key(|(path, _)| path.to_lowercase());
        let total = by_file.len();

        // Apply pagination.
        let page_start = offset.min(total);
        let page_end = limit.map(|l| (page_start + l).min(total)).unwrap_or(total);
        let by_file = &by_file[page_start..page_end];

        if json_output {
            #[derive(serde::Serialize)]
            struct FileExportsJson {
                file: String,
                exports: Vec<ExportJson>,
            }
            let json: Vec<FileExportsJson> = by_file
                .iter()
                .map(|(file, entry)| FileExportsJson {
                    file: file.to_string(),
                    exports: entry
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
                        .collect(),
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            println!(
                "{}",
                crate::format::format_list_exports_all(by_file, total, page_start)
            );
        }
    }

    Ok(())
}
