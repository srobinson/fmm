//! `fmm_list_exports` tool implementation.

use crate::manifest::Manifest;
use crate::mcp::args::ListExportsArgs;
use serde_json::Value;

pub(in crate::mcp) fn tool_list_exports(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    const DEFAULT_LIMIT: usize = 200;

    let args: ListExportsArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let dir = args.directory.as_deref();
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);

    if let Some(ref file_path) = args.file {
        let entry = manifest
            .files
            .get(file_path)
            .ok_or_else(|| format!("File '{}' not found in manifest", file_path))?;
        Ok(crate::format::format_list_exports_file(file_path, entry))
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
            .filter(|(name, path)| {
                if let Some(d) = dir
                    && !path.starts_with(d)
                {
                    return false;
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
        // Also include method_index matches (dotted names like "ClassName.method").
        for (dotted_name, loc) in &manifest.method_index {
            if !matcher(dotted_name) {
                continue;
            }
            if let Some(d) = dir
                && !loc.file.starts_with(d)
            {
                continue;
            }
            let lines = loc.lines.as_ref().map(|l| [l.start, l.end]);
            matches.push((dotted_name.clone(), loc.file.clone(), lines));
        }
        matches.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let total = matches.len();
        let page: Vec<(String, String, Option<[usize; 2]>)> =
            matches.into_iter().skip(offset).take(limit).collect();
        Ok(crate::format::format_list_exports_pattern(
            &page, total, offset,
        ))
    } else {
        let mut by_file: Vec<(&str, &crate::manifest::FileEntry)> = manifest
            .files
            .iter()
            .filter(|(path, entry)| {
                if let Some(d) = dir
                    && !path.starts_with(d)
                {
                    return false;
                }
                !entry.exports.is_empty()
            })
            .map(|(path, entry)| (path.as_str(), entry))
            .collect();
        by_file.sort_by_key(|(path, _)| path.to_lowercase());
        let total = by_file.len();
        let page: Vec<(&str, &crate::manifest::FileEntry)> =
            by_file.into_iter().skip(offset).take(limit).collect();
        Ok(crate::format::format_list_exports_all(&page, total, offset))
    }
}
