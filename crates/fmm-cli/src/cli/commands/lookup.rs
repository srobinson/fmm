use anyhow::{Context, Result};
use colored::Colorize;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct LookupExportJson {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

#[derive(serde::Serialize)]
struct LookupJson {
    symbol: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exports: Vec<LookupExportJson>,
    imports: Vec<String>,
    dependencies: Vec<String>,
    loc: usize,
}

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

    // Check export_all for additional definitions (collision detection).
    let collision_note = if let Some(all) = manifest.export_all.get(symbol) {
        let others: Vec<&str> = all
            .iter()
            .map(|loc| loc.file.as_str())
            .filter(|f| *f != file.as_str())
            .collect();
        if others.is_empty() {
            None
        } else {
            let file_list = others.join(", ");
            Some(format!(
                "⚠ {} additional definition(s) found: [{}] — use fmm_glossary for full collision analysis",
                others.len(),
                file_list
            ))
        }
    } else {
        None
    };

    if json_output {
        let json = LookupJson {
            symbol: symbol.to_string(),
            file,
            lines: symbol_lines.as_ref().map(|l| [l.start, l.end]),
            exports: entry_exports_json(entry),
            imports: entry.imports.clone(),
            dependencies: entry.dependencies.clone(),
            loc: entry.loc,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            fmm_core::format::format_lookup_export(
                symbol,
                &file,
                symbol_lines.as_ref(),
                entry,
                collision_note.as_deref(),
            )
        );
    }

    Ok(())
}

fn entry_exports_json(entry: &fmm_core::manifest::FileEntry) -> Vec<LookupExportJson> {
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
            LookupExportJson {
                name: name.clone(),
                lines,
            }
        })
        .collect()
}
