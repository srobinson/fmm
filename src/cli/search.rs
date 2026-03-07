use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::Manifest;

// -- JSON output types (for --json flag only) --

#[derive(serde::Serialize)]
struct ExportMatchJson {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

#[derive(serde::Serialize)]
struct BareSearchJson {
    exports: Vec<ExportMatchJson>,
    files: Vec<String>,
    imports: Vec<ImportMatchJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    call_sites: Vec<NamedImportMatchJson>,
}

#[derive(serde::Serialize)]
struct ImportMatchJson {
    package: String,
    files: Vec<String>,
}

#[derive(serde::Serialize)]
struct NamedImportMatchJson {
    symbol: String,
    source: String,
    files: Vec<String>,
}

#[derive(serde::Serialize)]
struct FlagSearchJson {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exports: Option<Vec<ExportWithLinesJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loc: Option<usize>,
}

#[derive(serde::Serialize)]
struct ExportWithLinesJson {
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
    directory: Option<String>,
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
        if has_flags {
            // Combined: intersect term results with the filter file set (AND semantics).
            let (min_loc, max_loc) = if let Some(ref loc_expr) = loc {
                let (op, value) = parse_loc_expr(loc_expr)?;
                match op.as_str() {
                    ">" => (Some(value + 1), None),
                    ">=" => (Some(value), None),
                    "<" => (None, Some(value.saturating_sub(1))),
                    "<=" => (None, Some(value)),
                    "=" => (Some(value), Some(value)),
                    _ => (None, None),
                }
            } else {
                (None, None)
            };
            let filters = crate::search::SearchFilters {
                export,
                imports,
                depends_on,
                min_loc,
                max_loc,
            };
            let filter_results = crate::search::filter_search(&manifest, &filters);
            let filter_files: std::collections::HashSet<&str> =
                filter_results.iter().map(|r| r.file.as_str()).collect();
            let mut result = crate::search::bare_search(&manifest, search_term, None);
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
            if json_output {
                let json = BareSearchJson {
                    exports: result
                        .exports
                        .iter()
                        .map(|h| ExportMatchJson {
                            name: h.name.clone(),
                            file: h.file.clone(),
                            lines: h.lines,
                        })
                        .collect(),
                    files: result.files.clone(),
                    imports: result
                        .imports
                        .iter()
                        .map(|h| ImportMatchJson {
                            package: h.package.clone(),
                            files: h.files.clone(),
                        })
                        .collect(),
                    call_sites: result
                        .named_import_hits
                        .iter()
                        .map(|h| NamedImportMatchJson {
                            symbol: h.symbol.clone(),
                            source: h.source.clone(),
                            files: h.files.clone(),
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                let mut formatted = crate::format::format_bare_search(&result, true);
                if result.exports.is_empty() && !result.files.is_empty() {
                    formatted.push_str(&format!(
                        "\n[No exports matching '{}' found in the {} matching file{}]",
                        search_term,
                        result.files.len(),
                        if result.files.len() == 1 { "" } else { "s" }
                    ));
                }
                println!("{}", formatted);
            }
        } else {
            bare_search(&manifest, search_term, json_output)?;
        }
    } else if has_flags {
        flag_search(
            &manifest,
            export,
            imports,
            loc,
            depends_on,
            directory,
            json_output,
        )?;
    } else {
        flag_search(&manifest, None, None, None, None, None, json_output)?;
    }

    Ok(())
}

// -- Bare search --

fn bare_search(manifest: &Manifest, term: &str, json_output: bool) -> Result<()> {
    let result = crate::search::bare_search(manifest, term, None);

    if json_output {
        let json = BareSearchJson {
            exports: result
                .exports
                .iter()
                .map(|h| ExportMatchJson {
                    name: h.name.clone(),
                    file: h.file.clone(),
                    lines: h.lines,
                })
                .collect(),
            files: result.files.clone(),
            imports: result
                .imports
                .iter()
                .map(|h| ImportMatchJson {
                    package: h.package.clone(),
                    files: h.files.clone(),
                })
                .collect(),
            call_sites: result
                .named_import_hits
                .iter()
                .map(|h| NamedImportMatchJson {
                    symbol: h.symbol.clone(),
                    source: h.source.clone(),
                    files: h.files.clone(),
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let total = result.exports.len()
        + result.files.len()
        + result.imports.len()
        + result.named_import_hits.len();
    if total == 0 {
        println!("{} No matches for '{}'", "!".yellow(), term);
        println!(
            "\n  {} Try 'fmm search' with no args to list all indexed files",
            "hint:".cyan()
        );
        return Ok(());
    }

    // Use shared formatter with ANSI colors
    println!("{}", crate::format::format_bare_search(&result, true));

    Ok(())
}

// -- Flag-based search --

fn flag_search(
    manifest: &Manifest,
    export: Option<String>,
    imports: Option<String>,
    loc: Option<String>,
    depends_on: Option<String>,
    directory: Option<String>,
    json_output: bool,
) -> Result<()> {
    // For non-JSON export-only searches, use the column-aligned format
    if !json_output {
        if let Some(ref export_name) = export {
            if imports.is_none() && depends_on.is_none() && loc.is_none() {
                let dir = directory.as_deref();
                let matches: Vec<_> = crate::search::find_export_matches(manifest, export_name)
                    .into_iter()
                    .filter(|h| dir.is_none_or(|d| h.file.starts_with(d)))
                    .collect();
                if matches.is_empty() {
                    println!("{} No matching exports", "!".yellow());
                    println!(
                        "\n  {} Export search is case-insensitive. Try a shorter term or 'fmm search' to browse all",
                        "hint:".cyan()
                    );
                } else {
                    let tuples: Vec<(String, String, Option<[usize; 2]>)> = matches
                        .iter()
                        .map(|h| (h.name.clone(), h.file.clone(), h.lines))
                        .collect();
                    let total = tuples.len();
                    println!(
                        "{}",
                        crate::format::format_list_exports_pattern(&tuples, total, 0)
                    );
                }
                return Ok(());
            }
        }
    }

    // Convert LOC expression to min/max
    let (min_loc, max_loc) = if let Some(ref loc_expr) = loc {
        let (op, value) = parse_loc_expr(loc_expr)?;
        match op.as_str() {
            ">" => (Some(value + 1), None),
            ">=" => (Some(value), None),
            "<" => (None, Some(value.saturating_sub(1))),
            "<=" => (None, Some(value)),
            "=" => (Some(value), Some(value)),
            _ => (None, None),
        }
    } else {
        (None, None)
    };

    let filters = crate::search::SearchFilters {
        export: export.clone(),
        imports,
        depends_on,
        min_loc,
        max_loc,
    };
    let results = crate::search::filter_search(manifest, &filters);

    if json_output {
        // For export-only JSON, use the rich export format
        if let Some(ref export_name) = export {
            if filters.imports.is_none() && filters.depends_on.is_none() && loc.is_none() {
                let matches = crate::search::find_export_matches(manifest, export_name);
                let export_json: Vec<ExportMatchJson> = matches
                    .iter()
                    .map(|h| ExportMatchJson {
                        name: h.name.clone(),
                        file: h.file.clone(),
                        lines: h.lines,
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&export_json)?);
                return Ok(());
            }
        }

        let json: Vec<FlagSearchJson> = results
            .iter()
            .map(|r| FlagSearchJson {
                file: r.file.clone(),
                exports: if r.exports.is_empty() {
                    None
                } else {
                    Some(
                        r.exports
                            .iter()
                            .map(|e| ExportWithLinesJson {
                                name: e.name.clone(),
                                lines: e.lines,
                            })
                            .collect(),
                    )
                },
                imports: if r.imports.is_empty() {
                    None
                } else {
                    Some(r.imports.clone())
                },
                dependencies: if r.dependencies.is_empty() {
                    None
                } else {
                    Some(r.dependencies.clone())
                },
                loc: Some(r.loc),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else if results.is_empty() {
        println!("{} No matches found", "!".yellow());
    } else {
        println!("{} {} file(s) found:\n", "✓".green(), results.len());
        println!("{}", crate::format::format_filter_search(&results, true));
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
    use crate::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};

    fn test_manifest() -> Manifest {
        let mut m = Manifest::new();

        m.files.insert(
            "src/store/index.ts".to_string(),
            FileEntry {
                exports: vec!["createStore".to_string(), "destroyStore".to_string()],
                export_lines: Some(vec![
                    ExportLines { start: 12, end: 45 },
                    ExportLines { start: 47, end: 60 },
                ]),
                methods: None,
                imports: vec!["redux".to_string()],
                dependencies: vec!["./types".to_string()],
                loc: 120,
                modified: None,
                function_names: Vec::new(),
                ..Default::default()
            },
        );
        m.files.insert(
            "src/store/provider.tsx".to_string(),
            FileEntry {
                exports: vec!["StoreProvider".to_string()],
                export_lines: Some(vec![ExportLines { start: 8, end: 22 }]),
                methods: None,
                imports: vec!["react".to_string(), "redux".to_string()],
                dependencies: vec!["./index".to_string()],
                loc: 45,
                modified: None,
                function_names: Vec::new(),
                ..Default::default()
            },
        );
        m.files.insert(
            "src/hooks/useStore.ts".to_string(),
            FileEntry {
                exports: vec!["useStore".to_string()],
                export_lines: Some(vec![ExportLines { start: 3, end: 15 }]),
                methods: None,
                imports: vec!["react".to_string()],
                dependencies: vec!["../store/index".to_string()],
                loc: 30,
                modified: None,
                function_names: Vec::new(),
                ..Default::default()
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
                methods: None,
                imports: vec!["crypto".to_string()],
                dependencies: vec!["./session".to_string()],
                loc: 80,
                modified: None,
                function_names: Vec::new(),
                ..Default::default()
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
        let matches = crate::search::find_export_matches(&m, "createStore");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "createStore");
        assert_eq!(matches[0].file, "src/store/index.ts");
    }

    #[test]
    fn fuzzy_export_match_substring() {
        let m = test_manifest();
        let matches = crate::search::find_export_matches(&m, "store");
        assert!(matches.len() >= 3);
        let names: Vec<&str> = matches.iter().map(|h| h.name.as_str()).collect();
        assert!(names.contains(&"createStore"));
        assert!(names.contains(&"StoreProvider"));
        assert!(names.contains(&"useStore"));
    }

    #[test]
    fn fuzzy_export_match_case_insensitive() {
        let m = test_manifest();
        let matches = crate::search::find_export_matches(&m, "STORE");
        assert!(matches.len() >= 3);
        let names: Vec<&str> = matches.iter().map(|h| h.name.as_str()).collect();
        assert!(names.contains(&"createStore"));
        assert!(names.contains(&"useStore"));
    }

    #[test]
    fn export_no_match() {
        let m = test_manifest();
        let matches = crate::search::find_export_matches(&m, "xyznothing");
        assert!(matches.is_empty());
    }

    #[test]
    fn exact_match_ranked_first() {
        let m = test_manifest();
        let matches = crate::search::find_export_matches(&m, "createStore");
        assert_eq!(matches[0].name, "createStore");
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
    fn bare_search_produces_grouped_text() {
        let m = test_manifest();
        let result = crate::search::bare_search(&m, "store", None);
        let text = crate::format::format_bare_search(&result, false);
        assert!(text.contains("EXPORTS"));
        assert!(text.contains("createStore"));
    }

    #[test]
    fn filter_search_produces_per_file_text() {
        let m = test_manifest();
        let filters = crate::search::SearchFilters {
            export: None,
            imports: Some("redux".to_string()),
            depends_on: None,
            min_loc: None,
            max_loc: None,
        };
        let results = crate::search::filter_search(&m, &filters);
        let text = crate::format::format_filter_search(&results, false);
        assert!(text.contains("redux"));
        assert!(text.contains("imports:"));
    }

    fn test_manifest_with_named_imports() -> Manifest {
        use std::collections::HashMap;
        let mut m = Manifest::new();

        let mut named_a: HashMap<String, Vec<String>> = HashMap::new();
        named_a.insert(
            "@tanstack/react-start".to_string(),
            vec!["createServerFn".to_string(), "createFileRoute".to_string()],
        );
        m.files.insert(
            "src/api/routes.ts".to_string(),
            FileEntry {
                exports: vec!["routeHandler".to_string()],
                export_lines: None,
                methods: None,
                imports: vec!["@tanstack/react-start".to_string()],
                dependencies: vec![],
                loc: 50,
                modified: None,
                function_names: Vec::new(),
                named_imports: named_a,
                ..Default::default()
            },
        );

        let mut named_b: HashMap<String, Vec<String>> = HashMap::new();
        named_b.insert(
            "@tanstack/react-start".to_string(),
            vec!["createServerFn".to_string()],
        );
        m.files.insert(
            "src/api/actions.ts".to_string(),
            FileEntry {
                exports: vec!["submitAction".to_string()],
                export_lines: None,
                methods: None,
                imports: vec!["@tanstack/react-start".to_string()],
                dependencies: vec![],
                loc: 30,
                modified: None,
                function_names: Vec::new(),
                named_imports: named_b,
                ..Default::default()
            },
        );

        // File with no named imports of interest
        m.files.insert(
            "src/utils/helpers.ts".to_string(),
            FileEntry {
                exports: vec!["formatDate".to_string()],
                export_lines: None,
                methods: None,
                imports: vec!["date-fns".to_string()],
                dependencies: vec![],
                loc: 20,
                modified: None,
                function_names: Vec::new(),
                ..Default::default()
            },
        );

        for (path, entry) in &m.files {
            for export in &entry.exports {
                m.export_index.insert(export.clone(), path.clone());
                m.export_locations.insert(
                    export.clone(),
                    ExportLocation {
                        file: path.clone(),
                        lines: None,
                    },
                );
            }
        }

        m
    }

    #[test]
    fn named_import_exact_match() {
        let m = test_manifest_with_named_imports();
        let result = crate::search::bare_search(&m, "createServerFn", None);
        assert_eq!(result.named_import_hits.len(), 1);
        let hit = &result.named_import_hits[0];
        assert_eq!(hit.symbol, "createServerFn");
        assert_eq!(hit.source, "@tanstack/react-start");
        assert_eq!(hit.files.len(), 2);
        assert!(hit.files.contains(&"src/api/actions.ts".to_string()));
        assert!(hit.files.contains(&"src/api/routes.ts".to_string()));
    }

    #[test]
    fn named_import_fuzzy_match() {
        let m = test_manifest_with_named_imports();
        // "serverFn" is a case-insensitive substring of "createServerFn"
        let result = crate::search::bare_search(&m, "serverFn", None);
        assert!(!result.named_import_hits.is_empty());
        let hit = &result.named_import_hits[0];
        assert_eq!(hit.symbol, "createServerFn");
        // "createFileRoute" does not contain "serverFn"
        assert!(result
            .named_import_hits
            .iter()
            .all(|h| h.symbol != "createFileRoute"));
    }

    #[test]
    fn named_import_combined_mode_intersection() {
        let m = test_manifest_with_named_imports();
        let mut result = crate::search::bare_search(&m, "createServerFn", None);
        // Simulate combined mode: only allow src/api/actions.ts
        let allowed: std::collections::HashSet<&str> =
            ["src/api/actions.ts"].iter().copied().collect();
        result.named_import_hits.iter_mut().for_each(|h| {
            h.files.retain(|f| allowed.contains(f.as_str()));
        });
        result.named_import_hits.retain(|h| !h.files.is_empty());
        assert_eq!(result.named_import_hits.len(), 1);
        assert_eq!(
            result.named_import_hits[0].files,
            vec!["src/api/actions.ts"]
        );
    }

    #[test]
    fn named_import_appears_in_call_sites_section() {
        let m = test_manifest_with_named_imports();
        let result = crate::search::bare_search(&m, "createServerFn", None);
        let text = crate::format::format_bare_search(&result, false);
        assert!(text.contains("CALL SITES"));
        assert!(text.contains("createServerFn"));
        assert!(text.contains("@tanstack/react-start"));
    }
}
