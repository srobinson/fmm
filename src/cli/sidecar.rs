use anyhow::Result;
use colored::Colorize;
use rayon::prelude::*;

use crate::config::Config;
use crate::extractor::{sidecar_path_for, FileProcessor};

use super::{collect_files, resolve_root};

/// Whether to create only new sidecars or overwrite all.
enum SidecarAction {
    Generate,
    Update,
}

/// Shared processing logic for generate and update commands.
fn process_files(path: &str, dry_run: bool, action: SidecarAction) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        if matches!(action, SidecarAction::Generate) {
            println!(
                "\n  {} Supported languages: {}",
                "hint:".cyan(),
                config
                    .languages
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        println!(
            "  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    println!("Found {} files to process", files.len());

    let processor = FileProcessor::new(&root);
    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let result = match action {
                SidecarAction::Generate => processor.generate(file, dry_run),
                SidecarAction::Update => processor.update(file, dry_run),
            };
            match result {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!(
                        "{} {}: {}\n  {} Check file permissions and encoding",
                        "error:".red().bold(),
                        file.display(),
                        e,
                        "hint:".cyan()
                    );
                    None
                }
            }
        })
        .collect();

    for (file, msg) in &results {
        let sidecar = sidecar_path_for(file);
        let display = sidecar.strip_prefix(&root).unwrap_or(&sidecar).display();
        println!("{} {}", "✓".green(), display);
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    let (verb, next_hint) = match action {
        SidecarAction::Generate => (
            if dry_run {
                "would be written"
            } else {
                "written"
            },
            "Run 'fmm validate' to verify, or 'fmm search --export <name>' to find symbols",
        ),
        SidecarAction::Update => (
            if dry_run {
                "would be updated"
            } else {
                "updated"
            },
            "Run 'fmm validate' to verify sidecars are consistent",
        ),
    };

    if !results.is_empty() {
        println!(
            "\n{} {} sidecar(s) {}",
            "✓".green().bold(),
            results.len(),
            verb
        );
        if !dry_run {
            println!("\n  {} {}", "next:".cyan(), next_hint);
        }
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn generate(path: &str, dry_run: bool) -> Result<()> {
    process_files(path, dry_run, SidecarAction::Generate)
}

pub fn update(path: &str, dry_run: bool) -> Result<()> {
    process_files(path, dry_run, SidecarAction::Update)
}

pub fn validate(path: &str) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        println!(
            "\n  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    println!("Validating {} files...", files.len());

    let processor = FileProcessor::new(&root);
    let invalid: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            match processor.validate(file) {
                Ok(true) => None,
                Ok(false) => {
                    let sidecar = sidecar_path_for(file);
                    let reason = if sidecar.exists() {
                        "sidecar out of date"
                    } else {
                        "missing sidecar"
                    };
                    Some((file.to_path_buf(), reason.to_string()))
                }
                Err(e) => Some((file.to_path_buf(), format!("Error: {}", e))),
            }
        })
        .collect();

    if invalid.is_empty() {
        println!("{} All sidecars are up to date!", "✓".green().bold());
        Ok(())
    } else {
        println!(
            "{} {} file(s) need updating:",
            "✗".red().bold(),
            invalid.len()
        );
        for (file, msg) in &invalid {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            println!("  {} {}: {}", "✗".red(), rel.display(), msg.dimmed());
        }
        println!(
            "\n  {} Run 'fmm update' to regenerate stale sidecars, or 'fmm generate' for missing ones",
            "fix:".cyan()
        );
        anyhow::bail!("Sidecar validation failed");
    }
}

pub fn clean(path: &str, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    let processor = FileProcessor::new(&root);
    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let sidecar = sidecar_path_for(file);
            if !sidecar.exists() {
                return None;
            }
            let display = sidecar
                .strip_prefix(&root)
                .unwrap_or(&sidecar)
                .display()
                .to_string();
            if dry_run {
                return Some((display, true));
            }
            match processor.clean(file) {
                Ok(true) => Some((display, true)),
                Ok(false) => None,
                Err(e) => {
                    eprintln!(
                        "{} {}: {}\n  {} Check file permissions",
                        "error:".red().bold(),
                        file.display(),
                        e,
                        "hint:".cyan()
                    );
                    None
                }
            }
        })
        .collect();

    for (display, _) in &results {
        if dry_run {
            println!("  Would remove: {}", display);
        } else {
            println!("{} Removed {}", "✓".green(), display);
        }
    }

    // Also clean legacy .fmm/ directory
    let legacy_dir = root.join(".fmm");
    if legacy_dir.is_dir() {
        if dry_run {
            println!("  Would remove legacy directory: .fmm/");
        } else {
            std::fs::remove_dir_all(&legacy_dir)?;
            println!("{} Removed legacy .fmm/ directory", "✓".green());
        }
    }

    println!(
        "\n{} {} sidecar(s) {}",
        "✓".green().bold(),
        results.len(),
        if dry_run {
            "would be removed"
        } else {
            "removed"
        }
    );

    Ok(())
}
