use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest_ext::load_manifest;
use fmm_core::manifest::Manifest;

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
    named_imports: Vec<NamedImportMatchJson>,
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
    let manifest = load_manifest(&root)?;

    if manifest.files.is_empty() {
        println!(
            "{} No fmm index found in the current directory",
            "!".yellow()
        );
        println!(
            "\n  {} Run 'fmm generate' first to build the index",
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
            let filters = fmm_core::search::SearchFilters {
                export,
                imports,
                depends_on,
                min_loc,
                max_loc,
            };
            let filter_results = fmm_core::search::filter_search(&manifest, &filters);
            let filter_files: std::collections::HashSet<&str> =
                filter_results.iter().map(|r| r.file.as_str()).collect();
            let mut result = fmm_core::search::bare_search(&manifest, search_term, None);
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
                    named_imports: result
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
                let mut formatted = fmm_core::format::format_bare_search(&result, true);
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
    let result = fmm_core::search::bare_search(manifest, term, None);

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
            named_imports: result
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
    println!("{}", fmm_core::format::format_bare_search(&result, true));

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
    if !json_output
        && let Some(ref export_name) = export
        && imports.is_none()
        && depends_on.is_none()
        && loc.is_none()
    {
        let dir = directory.as_deref();
        let matches: Vec<_> = fmm_core::search::find_export_matches(manifest, export_name)
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
                fmm_core::format::format_list_exports_pattern(&tuples, total, 0)
            );
        }
        return Ok(());
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

    let filters = fmm_core::search::SearchFilters {
        export: export.clone(),
        imports,
        depends_on,
        min_loc,
        max_loc,
    };
    let results = fmm_core::search::filter_search(manifest, &filters);

    if json_output {
        // For export-only JSON, use the rich export format
        if let Some(ref export_name) = export
            && filters.imports.is_none()
            && filters.depends_on.is_none()
            && loc.is_none()
        {
            let matches = fmm_core::search::find_export_matches(manifest, export_name);
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
        println!("{}", fmm_core::format::format_filter_search(&results, true));
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
#[path = "search_tests.rs"]
mod tests;
