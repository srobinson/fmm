use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use rayon::prelude::*;

use crate::config::Config;
use crate::db;
use crate::extractor::{sidecar_path_for, FileProcessor};
use crate::resolver;

use super::{collect_files_multi, resolve_root_multi};

pub fn generate(paths: &[String], dry_run: bool, force: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
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
        println!(
            "  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    println!("Found {} files to process", files.len());

    let processor = FileProcessor::new(&root);

    // --- SQLite write path ---
    // Runs before the sidecar path so a fresh DB is available on next load.
    // Both paths are kept during this transition phase; ALP-917 removes sidecars.
    if !dry_run {
        let mut conn = db::open_or_create(&root)?;

        // Store workspace packages so the read path can resolve cross-package imports.
        let workspace_info = resolver::workspace::discover(&root);
        db::writer::upsert_workspace_packages(&conn, &workspace_info.packages)?;

        // Phase 1 (sequential): determine which files are stale in the DB.
        // mtime comparison is O(1) per file and fast even at 4,673 files.
        let dirty_files: Vec<&std::path::PathBuf> = files
            .iter()
            .filter(|file| {
                if force {
                    return true;
                }
                let rel = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .display()
                    .to_string();
                let mtime = db::writer::file_mtime_rfc3339(file);
                !db::writer::is_file_up_to_date(&conn, &rel, mtime.as_deref())
            })
            .collect();

        if !dirty_files.is_empty() {
            // Phase 2 (parallel): parse all stale files.
            let parse_results: Vec<(std::path::PathBuf, crate::parser::ParseResult)> = dirty_files
                .par_iter()
                .filter_map(|file| match processor.parse(file) {
                    Ok(result) => Some(((*file).clone(), result)),
                    Err(e) => {
                        eprintln!("{} {}: {}", "error:".red().bold(), file.display(), e);
                        None
                    }
                })
                .collect();

            // Phase 3 (transacted): write all parsed results to DB in one commit.
            {
                let tx = conn.transaction()?;
                for (abs_path, result) in &parse_results {
                    let rel = abs_path
                        .strip_prefix(&root)
                        .unwrap_or(abs_path)
                        .display()
                        .to_string();
                    let mtime = db::writer::file_mtime_rfc3339(abs_path);
                    db::writer::upsert_file_data(&tx, &rel, result, mtime.as_deref())?;
                }
                tx.commit()?;
            }

            // Phase 4: rebuild the pre-computed reverse dependency graph.
            db::writer::rebuild_and_write_reverse_deps(&mut conn, &root)?;
        }

        db::writer::write_meta(&conn, "fmm_version", env!("CARGO_PKG_VERSION"))?;
        db::writer::write_meta(&conn, "generated_at", &Utc::now().to_rfc3339())?;
    }

    // --- Sidecar write path (kept for backward compatibility) ---
    // Parses all files independently; shares no state with the SQLite path above.
    // Will be removed entirely in ALP-917.
    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| match processor.process(file, dry_run, force) {
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

    if !results.is_empty() {
        let verb = if dry_run {
            "would be written"
        } else {
            "written"
        };
        println!(
            "\n{} {} sidecar(s) {}",
            "✓".green().bold(),
            results.len(),
            verb
        );
        if !dry_run {
            println!(
                "\n  {} Run 'fmm validate' to verify, or 'fmm search --export <name>' to find symbols",
                "next:".cyan()
            );
        }
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn validate(paths: &[String]) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;

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
        .filter_map(|file| match processor.validate(file) {
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
            "\n  {} Run 'fmm generate' to regenerate stale sidecars",
            "fix:".cyan()
        );
        anyhow::bail!("Sidecar validation failed");
    }
}

pub fn clean(paths: &[String], dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;

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
