use anyhow::{Context, Result};
use colored::Colorize;

/// Search result for JSON output
#[derive(serde::Serialize)]
struct SearchResult {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loc: Option<usize>,
}

pub fn search(
    export: Option<String>,
    imports: Option<String>,
    loc: Option<String>,
    depends_on: Option<String>,
    json_output: bool,
) -> Result<()> {
    let root = std::env::current_dir()?;
    let manifest = crate::manifest::Manifest::load_from_sidecars(&root)?;

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

    let mut results: Vec<SearchResult> = Vec::new();

    // Search by export name (uses reverse index)
    if let Some(ref export_name) = export {
        if let Some(file_path) = manifest.export_index.get(export_name) {
            if let Some(entry) = manifest.files.get(file_path) {
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
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
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
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
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
            }
        }
    }

    // Filter by LOC
    if let Some(ref loc_expr) = loc {
        let (op, value) = parse_loc_expr(loc_expr)?;

        if export.is_none() && imports.is_none() && depends_on.is_none() {
            for (file_path, entry) in &manifest.files {
                if matches_loc_filter(entry.loc, &op, value) {
                    results.push(SearchResult {
                        file: file_path.clone(),
                        exports: Some(entry.exports.clone()),
                        imports: Some(entry.imports.clone()),
                        dependencies: Some(entry.dependencies.clone()),
                        loc: Some(entry.loc),
                    });
                }
            }
        } else {
            results.retain(|r| r.loc.is_some_and(|l| matches_loc_filter(l, &op, value)));
        }
    }

    // If no filters provided, list all files
    if export.is_none() && imports.is_none() && depends_on.is_none() && loc.is_none() {
        for (file_path, entry) in &manifest.files {
            results.push(SearchResult {
                file: file_path.clone(),
                exports: Some(entry.exports.clone()),
                imports: Some(entry.imports.clone()),
                dependencies: Some(entry.dependencies.clone()),
                loc: Some(entry.loc),
            });
        }
    }

    results.sort_by(|a, b| a.file.cmp(&b.file));

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() {
        println!("{} No matches found", "!".yellow());
        if export.is_some() {
            println!(
                "\n  {} Export names are case-sensitive. Try 'fmm search' with no filters to list all indexed files",
                "hint:".cyan()
            );
        }
    } else {
        println!("{} {} file(s) found:\n", "✓".green(), results.len());
        for result in &results {
            println!("{}", result.file.white().bold());
            if let Some(ref exports) = result.exports {
                if !exports.is_empty() {
                    println!("  {} {}", "exports:".dimmed(), exports.join(", "));
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
