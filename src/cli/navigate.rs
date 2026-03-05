use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::Manifest;

// -- JSON output structs --

#[derive(serde::Serialize)]
struct LookupJson {
    symbol: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
    imports: Vec<String>,
    dependencies: Vec<String>,
    loc: usize,
}

#[derive(serde::Serialize)]
struct ReadSymbolJson {
    symbol: String,
    file: String,
    lines: [usize; 2],
    source: String,
}

#[derive(serde::Serialize)]
struct DepsJson {
    file: String,
    local_deps: Vec<String>,
    external: Vec<String>,
    downstream: Vec<String>,
}

#[derive(serde::Serialize)]
struct OutlineExportJson {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

#[derive(serde::Serialize)]
struct OutlineJson {
    file: String,
    exports: Vec<OutlineExportJson>,
    loc: usize,
}

#[derive(serde::Serialize)]
struct ListFileJson {
    file: String,
    loc: usize,
    exports: usize,
}

#[derive(serde::Serialize)]
struct ExportJson {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

// -- Helper: load manifest --

fn load_manifest() -> Result<(std::path::PathBuf, Manifest)> {
    let root = std::env::current_dir().context("Failed to get current directory")?;
    let manifest = Manifest::load_from_sidecars(&root)?;
    Ok((root, manifest))
}

fn warn_no_sidecars() {
    println!(
        "{} No .fmm sidecars found. Run {} first.",
        "!".yellow(),
        "fmm generate".bold()
    );
}

// -- fmm lookup <symbol> --

pub fn lookup(symbol: &str, json_output: bool) -> Result<()> {
    let (_, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    let (file, symbol_lines) = if let Some(loc) = manifest.export_locations.get(symbol) {
        (loc.file.clone(), loc.lines.clone())
    } else if let Some(file_path) = manifest.export_index.get(symbol) {
        (file_path.clone(), None)
    } else if let Some(loc) = manifest.method_index.get(symbol) {
        (loc.file.clone(), loc.lines.clone())
    } else {
        anyhow::bail!(
            "Symbol '{}' not found. Try {} to fuzzy-search.",
            symbol,
            format!("fmm exports {}", symbol).bold()
        );
    };

    let entry = manifest
        .files
        .get(&file)
        .with_context(|| format!("File '{}' not found in manifest", file))?;

    if json_output {
        let json = LookupJson {
            symbol: symbol.to_string(),
            file,
            lines: symbol_lines.as_ref().map(|l| [l.start, l.end]),
            imports: entry.imports.clone(),
            dependencies: entry.dependencies.clone(),
            loc: entry.loc,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            crate::format::format_lookup_export(symbol, &file, symbol_lines.as_ref(), entry)
        );
    }

    Ok(())
}

// -- fmm read <symbol> --

pub fn read_symbol(name: &str, no_truncate: bool, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if name.trim().is_empty() {
        anyhow::bail!("Symbol name must not be empty.");
    }

    // Dotted notation: ClassName.method — look up in method_index.
    let (resolved_file, resolved_lines) = if name.contains('.') {
        let loc = manifest.method_index.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Method '{}' not found. Use {} to see available methods.",
                name,
                "fmm outline <file>".bold()
            )
        })?;
        (loc.file.clone(), loc.lines.clone())
    } else {
        let location = manifest.export_locations.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Export '{}' not found. Use {} to discover symbols.",
                name,
                format!("fmm exports {}", name).bold()
            )
        })?;

        // If the winning location is a re-export hub, find the concrete definition.
        if crate::mcp::tools::is_reexport_file(&location.file) {
            if let Some((concrete_file, concrete_lines)) =
                crate::mcp::tools::find_concrete_definition(&manifest, name, &location.file)
            {
                (concrete_file, Some(concrete_lines))
            } else {
                (location.file.clone(), location.lines.clone())
            }
        } else {
            (location.file.clone(), location.lines.clone())
        }
    };

    let lines = resolved_lines.ok_or_else(|| {
        anyhow::anyhow!(
            "No line range for '{}' in '{}' — regenerate sidecars with 'fmm generate'.",
            name,
            resolved_file
        )
    })?;

    let source_path = root.join(&resolved_file);
    let content = std::fs::read_to_string(&source_path)
        .with_context(|| format!("Cannot read '{}'", resolved_file))?;

    let source_lines: Vec<&str> = content.lines().collect();
    let start = lines.start.saturating_sub(1);
    let end = lines.end.min(source_lines.len());

    if start >= source_lines.len() {
        anyhow::bail!(
            "Line range [{}, {}] out of bounds for '{}' ({} lines)",
            lines.start,
            lines.end,
            resolved_file,
            source_lines.len()
        );
    }

    let mut symbol_source = source_lines[start..end].join("\n");

    // Apply 10KB cap unless --no-truncate is set.
    const TRUNCATE_CAP: usize = 10_240;
    if !no_truncate && symbol_source.len() > TRUNCATE_CAP {
        symbol_source.truncate(TRUNCATE_CAP);
        if let Some(pos) = symbol_source.rfind('\n') {
            symbol_source.truncate(pos);
        }
        symbol_source.push_str("\n... (truncated — use --no-truncate for full source)");
    }

    if json_output {
        let json = ReadSymbolJson {
            symbol: name.to_string(),
            file: resolved_file,
            lines: [lines.start, lines.end],
            source: symbol_source,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            crate::format::format_read_symbol(name, &resolved_file, &lines, &symbol_source)
        );
    }

    Ok(())
}

// -- fmm deps <file> --

pub fn deps(file: &str, depth: i32, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if depth != -1 && depth < 1 {
        anyhow::bail!("--depth must be >= 1 or -1 (full closure). Got {}.", depth);
    }

    if file.ends_with('/') || root.join(file).is_dir() {
        anyhow::bail!(
            "'{}' is a directory. Use {} to list files.",
            file,
            format!("fmm ls {}", file).bold()
        );
    }

    let entry = manifest.files.get(file).ok_or_else(|| {
        anyhow::anyhow!(
            "File '{}' not found in manifest. Run 'fmm generate' to index it.",
            file
        )
    })?;

    if json_output {
        if depth == 1 {
            let (local, external, downstream) =
                crate::search::dependency_graph(&manifest, file, entry);
            let json = DepsJson {
                file: file.to_string(),
                local_deps: local,
                external,
                downstream: downstream.into_iter().cloned().collect(),
            };
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            let (upstream, external, downstream) =
                crate::search::dependency_graph_transitive(&manifest, file, entry, depth);
            #[derive(serde::Serialize)]
            struct TransitiveEntry {
                file: String,
                depth: i32,
            }
            #[derive(serde::Serialize)]
            struct TransitiveDepsJson {
                file: String,
                upstream: Vec<TransitiveEntry>,
                external: Vec<String>,
                downstream: Vec<TransitiveEntry>,
            }
            let json = TransitiveDepsJson {
                file: file.to_string(),
                upstream: upstream
                    .iter()
                    .map(|(f, d)| TransitiveEntry {
                        file: f.clone(),
                        depth: *d,
                    })
                    .collect(),
                external,
                downstream: downstream
                    .iter()
                    .map(|(f, d)| TransitiveEntry {
                        file: f.clone(),
                        depth: *d,
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    } else if depth == 1 {
        let (local, external, downstream) = crate::search::dependency_graph(&manifest, file, entry);
        println!(
            "{}",
            crate::format::format_dependency_graph(file, entry, &local, &external, &downstream)
        );
    } else {
        let (upstream, external, downstream) =
            crate::search::dependency_graph_transitive(&manifest, file, entry, depth);
        println!(
            "{}",
            crate::format::format_dependency_graph_transitive(
                file,
                entry,
                &upstream,
                &external,
                &downstream,
                depth
            )
        );
    }

    Ok(())
}

// -- fmm outline <file> --

pub fn outline(file: &str, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if file.ends_with('/') || root.join(file).is_dir() {
        anyhow::bail!(
            "'{}' is a directory. Use {} to list files.",
            file,
            format!("fmm ls {}", file).bold()
        );
    }

    let entry = manifest.files.get(file).ok_or_else(|| {
        anyhow::anyhow!(
            "File '{}' not found in manifest. Run 'fmm generate' to index it.",
            file
        )
    })?;

    if json_output {
        let exports: Vec<OutlineExportJson> = entry
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
                OutlineExportJson {
                    name: name.clone(),
                    lines,
                }
            })
            .collect();
        let json = OutlineJson {
            file: file.to_string(),
            exports,
            loc: entry.loc,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", crate::format::format_file_outline(file, entry));
    }

    Ok(())
}

// -- fmm ls [dir] --

pub fn ls(
    directory: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let (_, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if !matches!(sort_by, "name" | "loc" | "exports") {
        anyhow::bail!(
            "Invalid --sort-by '{}'. Valid values: name, loc, exports.",
            sort_by
        );
    }
    if let Some(o) = order {
        if !matches!(o, "asc" | "desc") {
            anyhow::bail!("Invalid --order '{}'. Valid values: asc, desc.", o);
        }
    }

    let mut entries: Vec<(&str, usize, usize)> = manifest
        .files
        .iter()
        .filter(|(path, _)| {
            if let Some(d) = directory {
                path.starts_with(d)
            } else {
                true
            }
        })
        .map(|(path, entry)| (path.as_str(), entry.loc, entry.exports.len()))
        .collect();

    let desc = match sort_by {
        "loc" | "exports" => order != Some("asc"),
        _ => order == Some("desc"),
    };

    match sort_by {
        "loc" => {
            if desc {
                entries.sort_by(|(_, a, _), (_, b, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, a, _), (_, b, _)| a.cmp(b));
            }
        }
        "exports" => {
            if desc {
                entries.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, a), (_, _, b)| a.cmp(b));
            }
        }
        _ => {
            if desc {
                entries.sort_by(|(a, _, _), (b, _, _)| b.to_lowercase().cmp(&a.to_lowercase()));
            } else {
                entries.sort_by_key(|(path, _, _)| path.to_lowercase());
            }
        }
    }

    let total = entries.len();

    if json_output {
        let json: Vec<ListFileJson> = entries
            .iter()
            .map(|(file, loc, exports)| ListFileJson {
                file: file.to_string(),
                loc: *loc,
                exports: *exports,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            crate::format::format_list_files(directory, &entries, total, 0)
        );
    }

    Ok(())
}

// -- fmm exports [pattern] --

pub fn exports(pattern: Option<&str>, directory: Option<&str>, json_output: bool) -> Result<()> {
    let (_, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if let Some(pat) = pattern {
        let pat_lower = pat.to_lowercase();
        let mut matches: Vec<(String, String, Option<[usize; 2]>)> = manifest
            .export_index
            .iter()
            .filter(|(name, path)| {
                if let Some(d) = directory {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                name.to_lowercase().contains(&pat_lower)
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
            if !dotted_name.to_lowercase().contains(&pat_lower) {
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
                crate::format::format_list_exports_pattern(&matches, total, 0)
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
                crate::format::format_list_exports_all(&by_file, total, 0)
            );
        }
    }

    Ok(())
}
