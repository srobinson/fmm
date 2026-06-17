//! `fmm_list_exports` tool implementation.

use crate::mcp::args::ListExportsArgs;
use fmm_core::config::{Config, FileTypeFilter};
use fmm_core::manifest::{FileEntry, Manifest};
use serde_json::Value;

use super::common::missing_file_diagnostic;

pub(in crate::mcp) fn tool_list_exports(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    const DEFAULT_LIMIT: usize = 200;

    let args: ListExportsArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let dir = args.directory.as_deref();
    let filter = args.filter.as_deref().unwrap_or("all");
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);
    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }
    let config = Config::load_from_dir(root).unwrap_or_default();
    let file_filter = FileTypeFilter::parse(filter).unwrap_or(FileTypeFilter::All);
    let path_matches_dir = |path: &str| dir.is_none_or(|d| path.starts_with(d));
    let export_matches = |name: &str, path: &str| {
        path_matches_dir(path) && manifest.export_matches_filter(name, path, file_filter, &config)
    };

    if let Some(ref file_path) = args.file {
        if !manifest.files.contains_key(file_path) {
            return Err(missing_file_diagnostic(root, file_path));
        }
        let entry = if path_matches_dir(file_path) {
            manifest
                .filtered_file_entry(file_path, file_filter, &config)
                .unwrap_or_default()
        } else {
            FileEntry::default()
        };
        Ok(fmm_core::format::format_list_exports_file(
            file_path, &entry,
        ))
    } else if let Some(ref pat) = args.pattern {
        // Auto-detect regex: if the pattern contains any metacharacter, compile
        // it as a case-sensitive regex.  Plain patterns keep the existing
        // case-insensitive substring match so existing callers are unaffected.
        const METACHAR: &[char] = &['^', '$', '[', '(', '\\', '.', '*', '+', '?', '{'];
        let uses_regex = pat.chars().any(|c| METACHAR.contains(&c));
        let matcher: Box<dyn Fn(&str) -> bool> = if uses_regex {
            match regex::Regex::new(pat) {
                Ok(re) => Box::new(move |name: &str| re.is_match(name)),
                Err(e) => return Err(format!("Invalid pattern: {e}")),
            }
        } else {
            let pat_lower = pat.to_lowercase();
            Box::new(move |name: &str| name.to_lowercase().contains(&pat_lower))
        };

        let mut matches: Vec<(String, String, Option<[usize; 2]>)> = manifest
            .export_index
            .iter()
            .filter(|(name, path)| export_matches(name, path) && matcher(name))
            .map(|(name, path)| {
                let lines = manifest
                    .export_locations
                    .get(name)
                    .and_then(|loc| loc.lines.as_ref())
                    .map(|l| [l.start, l.end]);
                (name.clone(), path.clone(), lines)
            })
            .collect();
        // Also include method_index matches (dotted names like "ClassName.method").
        for (dotted_name, loc) in &manifest.method_index {
            if !matcher(dotted_name) {
                continue;
            }
            if !export_matches(dotted_name, &loc.file) {
                continue;
            }
            let lines = loc.lines.as_ref().map(|l| [l.start, l.end]);
            matches.push((dotted_name.clone(), loc.file.clone(), lines));
        }
        matches.sort_by_key(|a| a.0.to_lowercase());
        let total = matches.len();
        let page: Vec<(String, String, Option<[usize; 2]>)> =
            matches.into_iter().skip(offset).take(limit).collect();
        Ok(fmm_core::format::format_list_exports_pattern(
            &page, total, offset,
        ))
    } else {
        let mut by_file: Vec<(String, FileEntry)> = manifest
            .files
            .keys()
            .filter_map(|path| {
                if !path_matches_dir(path) {
                    return None;
                }
                let entry = manifest.filtered_file_entry(path, file_filter, &config)?;
                (!entry.exports.is_empty()).then(|| (path.clone(), entry))
            })
            .collect();
        by_file.sort_by_key(|(path, _)| path.to_lowercase());
        let total = by_file.len();
        let page: Vec<(String, FileEntry)> = by_file.into_iter().skip(offset).take(limit).collect();
        let page_refs: Vec<(&str, &FileEntry)> = page
            .iter()
            .map(|(file, entry)| (file.as_str(), entry))
            .collect();
        Ok(fmm_core::format::format_list_exports_all(
            &page_refs, total, offset,
        ))
    }
}
