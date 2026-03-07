use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use rayon::prelude::*;
use rusqlite::params;

use crate::config::Config;
use crate::db;
use crate::extractor::FileProcessor;
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

    if dry_run {
        // Dry run: show what would be indexed without touching the DB.
        let dirty_files: Vec<&std::path::PathBuf> = if let Ok(conn) = db::open_db(&root) {
            files
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
                .collect()
        } else {
            files.iter().collect()
        };

        for abs_path in &dirty_files {
            let rel = abs_path.strip_prefix(&root).unwrap_or(abs_path);
            println!("  {} Would index: {}", "✓".green(), rel.display());
        }
        if !dirty_files.is_empty() {
            println!(
                "\n{} {} file(s) would be indexed",
                "✓".green().bold(),
                dirty_files.len()
            );
        } else {
            println!("{} All files up to date", "✓".green());
        }
        println!("\n{} (dry run — nothing written)", "!".yellow());
        return Ok(());
    }

    // --- SQLite write path ---
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

        for abs_path in &dirty_files {
            let rel = abs_path.strip_prefix(&root).unwrap_or(abs_path);
            println!("{} {}", "✓".green(), rel.display());
        }
        println!(
            "\n{} {} file(s) indexed",
            "✓".green().bold(),
            dirty_files.len()
        );
        println!(
            "\n  {} Run 'fmm validate' to verify, or 'fmm search --export <name>' to find symbols",
            "next:".cyan()
        );
    } else {
        println!("{} All files up to date", "✓".green());
    }

    db::writer::write_meta(&conn, "fmm_version", env!("CARGO_PKG_VERSION"))?;
    db::writer::write_meta(&conn, "generated_at", &Utc::now().to_rfc3339())?;

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

    let db_path = root.join(db::DB_FILENAME);
    if !db_path.exists() {
        println!("{} No fmm database found", "✗".red().bold());
        println!("\n  {} Run 'fmm generate' first", "fix:".cyan());
        anyhow::bail!("Validation failed: no database");
    }

    let conn = db::open_db(&root)?;

    println!("Validating {} files against index...", files.len());

    let invalid: Vec<_> = files
        .iter()
        .filter_map(|file| {
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let mtime = db::writer::file_mtime_rfc3339(file);
            if db::writer::is_file_up_to_date(&conn, &rel, mtime.as_deref()) {
                None
            } else {
                let reason = conn
                    .query_row(
                        "SELECT indexed_at FROM files WHERE path = ?1",
                        params![rel],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
                    .map(|_| "stale".to_string())
                    .unwrap_or_else(|| "not indexed".to_string());
                Some((file.to_path_buf(), reason))
            }
        })
        .collect();

    if invalid.is_empty() {
        println!(
            "{} All {} files are indexed and up to date",
            "✓".green().bold(),
            files.len()
        );
        Ok(())
    } else {
        println!(
            "{} {} file(s) need re-indexing:",
            "✗".red().bold(),
            invalid.len()
        );
        for (file, msg) in &invalid {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            println!("  {} {}: {}", "✗".red(), rel.display(), msg.dimmed());
        }
        println!(
            "\n  {} Run 'fmm generate' to update the index",
            "fix:".cyan()
        );
        anyhow::bail!("Validation failed");
    }
}

pub fn clean(paths: &[String], dry_run: bool, delete_db: bool) -> Result<()> {
    let root = resolve_root_multi(paths)?;
    let db_path = root.join(db::DB_FILENAME);
    let legacy_dir = root.join(".fmm");

    let has_db = db_path.exists();
    let has_legacy = legacy_dir.is_dir();

    if !has_db && !has_legacy {
        println!("{} Nothing to clean — no fmm database found", "!".yellow());
        return Ok(());
    }

    if dry_run {
        if has_db {
            if delete_db {
                println!("  Would remove: {}", db::DB_FILENAME);
            } else {
                let conn = db::open_db(&root)?;
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
                    .unwrap_or(0);
                println!(
                    "  Would clear {} indexed file(s) from {}",
                    count,
                    db::DB_FILENAME
                );
            }
        }
        if has_legacy {
            println!("  Would remove legacy directory: .fmm/");
        }
        println!("\n{} (dry run — nothing removed)", "!".yellow());
        return Ok(());
    }

    if has_db {
        if delete_db {
            std::fs::remove_file(&db_path)?;
            println!("{} Removed {}", "✓".green(), db::DB_FILENAME);
        } else {
            let conn = db::open_db(&root)?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
                .unwrap_or(0);
            conn.execute_batch(
                "DELETE FROM files; DELETE FROM reverse_deps; DELETE FROM workspace_packages;",
            )?;
            println!(
                "{} Cleared {} file(s) from index ({})",
                "✓".green(),
                count,
                db::DB_FILENAME
            );
        }
    }

    if has_legacy {
        std::fs::remove_dir_all(&legacy_dir)?;
        println!("{} Removed legacy .fmm/ directory", "✓".green());
    }

    println!(
        "\n  {} Run 'fmm generate' to rebuild the index",
        "next:".cyan()
    );

    Ok(())
}
