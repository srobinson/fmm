use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::{GlossaryMode, Manifest};

pub fn glossary(pattern: Option<String>, mode: &str, json_output: bool) -> Result<()> {
    let pattern = pattern.as_deref().unwrap_or("").trim().to_string();
    if pattern.is_empty() {
        anyhow::bail!(
            "pattern is required — provide a symbol name or substring (e.g. 'run_dispatch', 'config')"
        );
    }

    let root = std::env::current_dir().context("Failed to get current directory")?;
    let manifest = Manifest::load_from_sidecars(&root)?;

    if manifest.files.is_empty() {
        eprintln!(
            "{} No sidecars found. Run {} first.",
            "warning:".yellow(),
            "fmm generate".bold()
        );
        return Ok(());
    }

    let mode = match mode {
        "tests" => GlossaryMode::Tests,
        "all" => GlossaryMode::All,
        _ => GlossaryMode::Source,
    };
    let mut entries = manifest.build_glossary(&pattern, mode);

    // ALP-785: For dotted method queries, refine used_by via call-site detection.
    if let Some(dot_pos) = pattern.rfind('.') {
        let method_name = &pattern[dot_pos + 1..];
        if !method_name.is_empty() {
            for entry in &mut entries {
                for source in &mut entry.sources {
                    let refined = crate::manifest::call_site_finder::find_call_sites(
                        &root,
                        method_name,
                        &source.used_by,
                    );
                    source.used_by = refined;
                }
            }
        }
    }

    if json_output {
        let json = serde_json::to_string_pretty(&entries)?;
        println!("{}", json);
        return Ok(());
    }

    if entries.is_empty() {
        println!("{} No exports matching '{}'", "→".dimmed(), pattern);
        return Ok(());
    }

    let multi_def_count = entries.iter().filter(|e| e.sources.len() > 1).count();

    for entry in &entries {
        println!("{}", entry.name.bold().cyan());
        for src in &entry.sources {
            let loc_str = match &src.lines {
                Some(l) if l.start > 0 => format!(" [{}-{}]", l.start, l.end),
                _ => String::new(),
            };
            println!(
                "  {} {}{}",
                "src:".dimmed(),
                src.file.green(),
                loc_str.dimmed()
            );
            if src.used_by.is_empty() {
                println!("  {} {}", "used_by:".dimmed(), "(none)".dimmed());
            } else {
                let files: Vec<String> =
                    src.used_by.iter().map(|f| f.yellow().to_string()).collect();
                println!("  {} {}", "used_by:".dimmed(), files.join(", "));
            }
        }
    }

    println!(
        "\n{} {} exports matched",
        "→".dimmed(),
        entries.len().to_string().bold()
    );
    if multi_def_count > 0 {
        println!(
            "  {} {} with multiple definitions",
            "→".dimmed(),
            multi_def_count.to_string().bold()
        );
    }

    Ok(())
}
