use anyhow::{Context, Result};
use colored::Colorize;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct ReadSymbolJson {
    symbol: String,
    file: String,
    lines: [usize; 2],
    source: String,
}

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
            crate::format::format_read_symbol(name, &resolved_file, &lines, &symbol_source, false)
        );
    }

    Ok(())
}
