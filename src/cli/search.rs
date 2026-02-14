use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::{ExportLocation, Manifest};

// -- JSON output types --

#[derive(serde::Serialize)]
struct ExportMatch {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

/// JSON output for bare search (grouped by type)
#[derive(serde::Serialize)]
struct BareSearchResult {
    exports: Vec<ExportMatch>,
    files: Vec<String>,
    imports: Vec<ImportMatch>,
}

#[derive(serde::Serialize)]
struct ImportMatch {
    package: String,
    files: Vec<String>,
}

/// JSON output for flag-based search (per-file)
#[derive(serde::Serialize)]
struct FlagSearchResult {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exports: Option<Vec<ExportWithLines>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loc: Option<usize>,
}

#[derive(serde::Serialize)]
struct ExportWithLines {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

pub fn search(
    term: Option<String>,
    export: Option<String>,
    imports: Option<String>,
    loc: Option<String>,
    depends_on: Option<String>,
    json_output: bool,
) -> Result<()> {
    let root = std::env::current_dir()?;
    let manifest = Manifest::load_from_sidecars(&root)?;

    if manifest.files.is_empty() {
        println!(
            "{} No .fmm sidecars found in the current directory",
            "!".yellow()
        );
        println!(
            "\n  {} fmm search queries sidecar metadata. Run 'fmm generate' first to create them",
            "hint:".cyan()
        );
        return Ok(());
    }

    let has_flags = export.is_some() || imports.is_some() || depends_on.is_some() || loc.is_some();

    if let Some(ref search_term) = term {
        bare_search(&manifest, search_term, json_output)?;
    } else if has_flags {
        flag_search(&manifest, export, imports, loc, depends_on, json_output)?;
    } else {
        flag_search(&manifest, None, None, None, None, json_output)?;
    }

    Ok(())
}

// -- Bare search: searches everything, groups by type --

fn bare_search(manifest: &Manifest, term: &str, json_output: bool) -> Result<()> {
    let term_lower = term.to_lowercase();

    // 1. Exact export match (O(1))
    let mut export_matches: Vec<(&str, &ExportLocation)> = Vec::new();
    let mut seen_exports = std::collections::HashSet::new();

    if let Some(loc) = manifest.export_locations.get(term) {
        export_matches.push((term, loc));
        seen_exports.insert(term.to_string());
    }

    // 2. Fuzzy export match (case-insensitive substring)
    let mut fuzzy_exports: Vec<(&str, &ExportLocation)> = Vec::new();
    for (name, loc) in &manifest.export_locations {
        if seen_exports.contains(name.as_str()) {
            continue;
        }
        if name.to_lowercase().contains(&term_lower) {
            fuzzy_exports.push((name.as_str(), loc));
        }
    }
    fuzzy_exports.sort_by_key(|(name, _)| name.to_lowercase());
    export_matches.extend(fuzzy_exports);

    // 3. File path match
    let mut file_matches: Vec<&str> = manifest
        .files
        .keys()
        .filter(|path| path.to_lowercase().contains(&term_lower))
        .map(|s| s.as_str())
        .collect();
    file_matches.sort();

    // 4. Import match — find unique packages matching the term, with their files
    let mut import_map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (file_path, entry) in &manifest.files {
        for imp in &entry.imports {
            if imp.to_lowercase().contains(&term_lower) {
                import_map
                    .entry(imp.clone())
                    .or_default()
                    .push(file_path.clone());
            }
        }
    }
    for files in import_map.values_mut() {
        files.sort();
    }

    let total = export_matches.len() + file_matches.len() + import_map.len();

    if json_output {
        let result = BareSearchResult {
            exports: export_matches
                .iter()
                .map(|(name, loc)| ExportMatch {
                    name: name.to_string(),
                    file: loc.file.clone(),
                    lines: loc.lines.as_ref().map(|l| [l.start, l.end]),
                })
                .collect(),
            files: file_matches.iter().map(|s| s.to_string()).collect(),
            imports: import_map
                .into_iter()
                .map(|(pkg, files)| ImportMatch {
                    package: pkg,
                    files,
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if total == 0 {
        println!("{} No matches for '{}'", "!".yellow(), term);
        println!(
            "\n  {} Try 'fmm search' with no args to list all indexed files",
            "hint:".cyan()
        );
        return Ok(());
    }

    if !export_matches.is_empty() {
        println!("{}", "EXPORTS".bold());
        let name_width = export_matches
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);
        let file_width = export_matches
            .iter()
            .map(|(_, loc)| loc.file.len())
            .max()
            .unwrap_or(0);

        for (name, loc) in &export_matches {
            let lines_str = match &loc.lines {
                Some(l) => format!("[{}, {}]", l.start, l.end).dimmed().to_string(),
                None => String::new(),
            };
            println!(
                "  {:<nw$}  {:<fw$}  {}",
                name.white().bold(),
                loc.file,
                lines_str,
                nw = name_width,
                fw = file_width,
            );
        }
        println!();
    }

    if !file_matches.is_empty() {
        println!("{}", "FILES".bold());
        for path in &file_matches {
            println!("  {}", path);
        }
        println!();
    }

    if !import_map.is_empty() {
        println!("{}", "IMPORTS".bold());
        for (pkg, files) in &import_map {
            println!(
                "  {}  {}",
                pkg.white().bold(),
                format!(
                    "({} file{})",
                    files.len(),
                    if files.len() == 1 { "" } else { "s" }
                )
                .dimmed()
            );
        }
        println!();
    }

    Ok(())
}

// -- Flag-based search: existing behavior, enhanced with fuzzy exports + rich output --

fn flag_search(
    manifest: &Manifest,
    export: Option<String>,
    imports: Option<String>,
    loc: Option<String>,
    depends_on: Option<String>,
    json_output: bool,
) -> Result<()> {
    let mut results: Vec<FlagSearchResult> = Vec::new();

    // Search by export name — exact first, then fuzzy
    if let Some(ref export_name) = export {
        let matches = find_export_matches(manifest, export_name);

        if matches.is_empty() && !json_output {
            print_export_matches(&matches, json_output);
            if imports.is_none() && depends_on.is_none() && loc.is_none() {
                return Ok(());
            }
        } else if json_output {
            for (name, loc) in &matches {
                if results.iter().any(|r| r.file == loc.file) {
                    continue;
                }
                if let Some(entry) = manifest.files.get(&loc.file) {
                    results.push(file_to_flag_result(&loc.file, entry));
                }
                // For non-JSON we handle export display separately below
                let _ = name;
            }
        } else {
            // Rich export output — show matching exports with line ranges
            print_export_matches(&matches, json_output);
            // If only --export (no other flags), we're done after printing
            if imports.is_none() && depends_on.is_none() && loc.is_none() {
                return Ok(());
            }
            // If other flags are combined, continue to filter
            for (_, eloc) in &matches {
                if results.iter().any(|r| r.file == eloc.file) {
                    continue;
                }
                if let Some(entry) = manifest.files.get(&eloc.file) {
                    results.push(file_to_flag_result(&eloc.file, entry));
                }
            }
        }
    }

    // Search by imports
    if let Some(ref import_name) = imports {
        for (file_path, entry) in &manifest.files {
            if entry
                .imports
                .iter()
                .any(|i| i.contains(import_name.as_str()))
            {
                if results.iter().any(|r| r.file == *file_path) {
                    continue;
                }
                results.push(file_to_flag_result(file_path, entry));
            }
        }
    }

    // Search by dependencies
    if let Some(ref dep_path) = depends_on {
        for (file_path, entry) in &manifest.files {
            if entry
                .dependencies
                .iter()
                .any(|d| d.contains(dep_path.as_str()))
            {
                if results.iter().any(|r| r.file == *file_path) {
                    continue;
                }
                results.push(file_to_flag_result(file_path, entry));
            }
        }
    }

    // Filter by LOC
    if let Some(ref loc_expr) = loc {
        let (op, value) = parse_loc_expr(loc_expr)?;

        if export.is_none() && imports.is_none() && depends_on.is_none() {
            for (file_path, entry) in &manifest.files {
                if matches_loc_filter(entry.loc, &op, value) {
                    results.push(file_to_flag_result(file_path, entry));
                }
            }
        } else {
            results.retain(|r| r.loc.is_some_and(|l| matches_loc_filter(l, &op, value)));
        }
    }

    // If no filters provided, list all files
    if export.is_none() && imports.is_none() && depends_on.is_none() && loc.is_none() {
        for (file_path, entry) in &manifest.files {
            results.push(file_to_flag_result(file_path, entry));
        }
    }

    results.sort_by(|a, b| a.file.cmp(&b.file));

    if json_output {
        // For export-only searches, use the rich export JSON format
        if let Some(ref export_name) = export {
            if imports.is_none() && depends_on.is_none() && loc.is_none() {
                let matches = find_export_matches(manifest, export_name);
                let export_results: Vec<ExportMatch> = matches
                    .iter()
                    .map(|(name, eloc)| ExportMatch {
                        name: name.clone(),
                        file: eloc.file.clone(),
                        lines: eloc.lines.as_ref().map(|l| [l.start, l.end]),
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&export_results)?);
                return Ok(());
            }
        }
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() && export.is_none() {
        println!("{} No matches found", "!".yellow());
    } else if !results.is_empty() {
        println!("{} {} file(s) found:\n", "✓".green(), results.len());
        for result in &results {
            println!("{}", result.file.white().bold());
            if let Some(ref exports) = result.exports {
                if !exports.is_empty() {
                    let formatted: Vec<String> = exports
                        .iter()
                        .map(|e| match e.lines {
                            Some([s, end]) if s > 0 => format!("{} [{}, {}]", e.name, s, end),
                            _ => e.name.clone(),
                        })
                        .collect();
                    println!("  {} {}", "exports:".dimmed(), formatted.join(", "));
                }
            }
            if let Some(ref imports) = result.imports {
                if !imports.is_empty() {
                    println!("  {} {}", "imports:".dimmed(), imports.join(", "));
                }
            }
            if let Some(loc_val) = result.loc {
                println!("  {} {}", "loc:".dimmed(), loc_val);
            }
            println!();
        }
    }

    Ok(())
}

/// Find exports matching a name — exact O(1) first, then case-insensitive substring.
fn find_export_matches<'a>(
    manifest: &'a Manifest,
    name: &str,
) -> Vec<(String, &'a ExportLocation)> {
    let mut matches: Vec<(String, &ExportLocation)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Exact match (O(1))
    if let Some(loc) = manifest.export_locations.get(name) {
        matches.push((name.to_string(), loc));
        seen.insert(name.to_string());
    }

    // Fuzzy: case-insensitive substring
    let name_lower = name.to_lowercase();
    let mut fuzzy: Vec<(String, &ExportLocation)> = manifest
        .export_locations
        .iter()
        .filter(|(export_name, _)| {
            !seen.contains(export_name.as_str()) && export_name.to_lowercase().contains(&name_lower)
        })
        .map(|(export_name, loc)| (export_name.clone(), loc))
        .collect();
    fuzzy.sort_by(|(a, _), (b, _)| a.to_lowercase().cmp(&b.to_lowercase()));
    matches.extend(fuzzy);

    matches
}

/// Print export matches in the rich aligned format.
fn print_export_matches(matches: &[(String, &ExportLocation)], _json_output: bool) {
    if matches.is_empty() {
        println!("{} No matching exports", "!".yellow());
        println!(
            "\n  {} Export search is case-insensitive. Try a shorter term or 'fmm search' to browse all",
            "hint:".cyan()
        );
        return;
    }

    let name_width = matches
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    let file_width = matches
        .iter()
        .map(|(_, loc)| loc.file.len())
        .max()
        .unwrap_or(0);

    for (name, loc) in matches {
        let lines_str = match &loc.lines {
            Some(l) => format!("[{}, {}]", l.start, l.end).dimmed().to_string(),
            None => String::new(),
        };
        println!(
            "  {:<nw$}  {:<fw$}  {}",
            name.white().bold(),
            loc.file,
            lines_str,
            nw = name_width,
            fw = file_width,
        );
    }
}

fn file_to_flag_result(path: &str, entry: &crate::manifest::FileEntry) -> FlagSearchResult {
    let exports: Vec<ExportWithLines> = entry
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
            ExportWithLines {
                name: name.clone(),
                lines,
            }
        })
        .collect();

    FlagSearchResult {
        file: path.to_string(),
        exports: if exports.is_empty() {
            None
        } else {
            Some(exports)
        },
        imports: if entry.imports.is_empty() {
            None
        } else {
            Some(entry.imports.clone())
        },
        dependencies: if entry.dependencies.is_empty() {
            None
        } else {
            Some(entry.dependencies.clone())
        },
        loc: Some(entry.loc),
    }
}

fn parse_loc_expr(expr: &str) -> Result<(String, usize)> {
    let expr = expr.trim();

    if let Some(rest) = expr.strip_prefix(">=") {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok((">=".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix("<=") {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("<=".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('>') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok((">".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('<') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("<".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('=') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("=".to_string(), value))
    } else {
        let value: usize = expr
            .parse()
            .context("Invalid LOC expression. Use: >500, <100, =200, >=50, <=1000")?;
        Ok(("=".to_string(), value))
    }
}

fn matches_loc_filter(loc: usize, op: &str, value: usize) -> bool {
    match op {
        ">" => loc > value,
        "<" => loc < value,
        ">=" => loc >= value,
        "<=" => loc <= value,
        "=" => loc == value,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};

    fn test_manifest() -> Manifest {
        let mut m = Manifest::new();

        // Simulate adding files with exports
        m.files.insert(
            "src/store/index.ts".to_string(),
            FileEntry {
                exports: vec!["createStore".to_string(), "destroyStore".to_string()],
                export_lines: Some(vec![
                    ExportLines { start: 12, end: 45 },
                    ExportLines { start: 47, end: 60 },
                ]),
                imports: vec!["redux".to_string()],
                dependencies: vec!["./types".to_string()],
                loc: 120,
            },
        );
        m.files.insert(
            "src/store/provider.tsx".to_string(),
            FileEntry {
                exports: vec!["StoreProvider".to_string()],
                export_lines: Some(vec![ExportLines { start: 8, end: 22 }]),
                imports: vec!["react".to_string(), "redux".to_string()],
                dependencies: vec!["./index".to_string()],
                loc: 45,
            },
        );
        m.files.insert(
            "src/hooks/useStore.ts".to_string(),
            FileEntry {
                exports: vec!["useStore".to_string()],
                export_lines: Some(vec![ExportLines { start: 3, end: 15 }]),
                imports: vec!["react".to_string()],
                dependencies: vec!["../store/index".to_string()],
                loc: 30,
            },
        );
        m.files.insert(
            "src/auth/login.ts".to_string(),
            FileEntry {
                exports: vec!["login".to_string(), "logout".to_string()],
                export_lines: Some(vec![
                    ExportLines { start: 5, end: 20 },
                    ExportLines { start: 22, end: 35 },
                ]),
                imports: vec!["crypto".to_string()],
                dependencies: vec!["./session".to_string()],
                loc: 80,
            },
        );

        // Build export index and locations
        for (path, entry) in &m.files {
            for (i, export) in entry.exports.iter().enumerate() {
                m.export_index.insert(export.clone(), path.clone());
                let lines = entry
                    .export_lines
                    .as_ref()
                    .and_then(|el| el.get(i))
                    .cloned();
                m.export_locations.insert(
                    export.clone(),
                    ExportLocation {
                        file: path.clone(),
                        lines,
                    },
                );
            }
        }

        m
    }

    #[test]
    fn exact_export_match() {
        let m = test_manifest();
        let matches = find_export_matches(&m, "createStore");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "createStore");
        assert_eq!(matches[0].1.file, "src/store/index.ts");
    }

    #[test]
    fn fuzzy_export_match_substring() {
        let m = test_manifest();
        let matches = find_export_matches(&m, "store");
        // Should match: createStore, destroyStore, StoreProvider, useStore
        assert!(matches.len() >= 3);
        let names: Vec<&str> = matches.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"createStore"));
        assert!(names.contains(&"StoreProvider"));
        assert!(names.contains(&"useStore"));
    }

    #[test]
    fn fuzzy_export_match_case_insensitive() {
        let m = test_manifest();
        let matches = find_export_matches(&m, "STORE");
        assert!(matches.len() >= 3);
        let names: Vec<&str> = matches.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"createStore"));
        assert!(names.contains(&"useStore"));
    }

    #[test]
    fn export_no_match() {
        let m = test_manifest();
        let matches = find_export_matches(&m, "xyznothing");
        assert!(matches.is_empty());
    }

    #[test]
    fn exact_match_ranked_first() {
        let m = test_manifest();
        let matches = find_export_matches(&m, "createStore");
        assert_eq!(matches[0].0, "createStore");
        // Only exact match, no fuzzy (since "createStore" is a full name)
        // But destroyStore also contains "store" — exact should be first
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn parse_loc_operators() {
        assert_eq!(parse_loc_expr(">500").unwrap(), (">".to_string(), 500));
        assert_eq!(parse_loc_expr("<100").unwrap(), ("<".to_string(), 100));
        assert_eq!(parse_loc_expr(">=50").unwrap(), (">=".to_string(), 50));
        assert_eq!(parse_loc_expr("<=1000").unwrap(), ("<=".to_string(), 1000));
        assert_eq!(parse_loc_expr("=200").unwrap(), ("=".to_string(), 200));
        assert_eq!(parse_loc_expr("200").unwrap(), ("=".to_string(), 200));
    }

    #[test]
    fn loc_filter_matches() {
        assert!(matches_loc_filter(600, ">", 500));
        assert!(!matches_loc_filter(500, ">", 500));
        assert!(matches_loc_filter(50, "<", 100));
        assert!(matches_loc_filter(100, ">=", 100));
        assert!(matches_loc_filter(200, "=", 200));
    }

    #[test]
    fn file_to_flag_result_includes_line_ranges() {
        let entry = FileEntry {
            exports: vec!["foo".to_string()],
            export_lines: Some(vec![ExportLines { start: 5, end: 20 }]),
            imports: vec!["bar".to_string()],
            dependencies: vec![],
            loc: 100,
        };
        let result = file_to_flag_result("test.ts", &entry);
        assert_eq!(result.file, "test.ts");
        let exports = result.exports.unwrap();
        assert_eq!(exports[0].name, "foo");
        assert_eq!(exports[0].lines, Some([5, 20]));
    }
}
