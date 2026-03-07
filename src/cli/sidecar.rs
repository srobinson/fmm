use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use indicatif::ParallelProgressIterator;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rusqlite::params;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::db;
use crate::extractor::FileProcessor;
use crate::resolver;

use super::{collect_files_multi, resolve_root_multi};

/// Show progress bars when at least this many files need processing.
const PROGRESS_THRESHOLD: usize = 10;

pub fn generate(paths: &[String], dry_run: bool, force: bool, quiet: bool) -> Result<()> {
    let total_start = Instant::now();
    let config = Config::load().unwrap_or_default();

    // Scan phase: spinner while walking the directory tree.
    let scan_sp = if !quiet {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::with_template("{spinner:.blue} Scanning files...")
                .expect("valid template"),
        );
        sp.enable_steady_tick(Duration::from_millis(80));
        Some(sp)
    } else {
        None
    };

    let files = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;

    if let Some(sp) = &scan_sp {
        sp.finish_and_clear();
    }

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

    // Phase 1: bulk staleness check.
    // Load all indexed_at times in one query (avoids 39k individual SELECTs),
    // then compare in parallel with rayon (mtime syscalls are I/O-parallel).
    let phase1_start = Instant::now();
    let indexed_mtimes: std::collections::HashMap<String, String> = if !force {
        db::writer::load_indexed_mtimes(&conn)?
    } else {
        std::collections::HashMap::new()
    };
    let dirty_files: Vec<&std::path::PathBuf> = files
        .par_iter()
        .filter(|file| {
            if force {
                return true;
            }
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let Some(mtime) = db::writer::file_mtime_rfc3339(file) else {
                return true; // unreadable mtime → treat as dirty
            };
            // Dirty when not in DB, or stored indexed_at < file mtime.
            indexed_mtimes
                .get(&rel)
                .map(|indexed_at| indexed_at.as_str() < mtime.as_str())
                .unwrap_or(true)
        })
        .collect();
    let phase1_elapsed = phase1_start.elapsed();

    if dirty_files.is_empty() {
        let elapsed = total_start.elapsed();
        println!(
            "Found {} files · all up to date  ({:.1}s)",
            files.len(),
            elapsed.as_secs_f64()
        );
        db::writer::write_meta(&conn, "fmm_version", env!("CARGO_PKG_VERSION"))?;
        db::writer::write_meta(&conn, "generated_at", &Utc::now().to_rfc3339())?;
        return Ok(());
    }

    let show_progress = !quiet && dirty_files.len() >= PROGRESS_THRESHOLD;

    if !quiet {
        println!(
            "Found {} files · {} changed",
            files.len(),
            dirty_files.len()
        );
    }

    let processor = FileProcessor::new(&root);

    // Phase 2 (parallel): parse all stale files.
    let phase2_start = Instant::now();
    let parse_results: Vec<(std::path::PathBuf, crate::parser::ParseResult)> = if show_progress {
        let pb = ProgressBar::new(dirty_files.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "Parsing  {wide_bar:.cyan/blue} {pos}/{len}  {per_sec}  ETA {eta}",
            )
            .expect("valid template"),
        );
        let results = dirty_files
            .par_iter()
            .progress_with(pb.clone())
            .filter_map(|file| match processor.parse(file) {
                Ok(result) => Some(((*file).clone(), result)),
                Err(e) => {
                    eprintln!("{} {}: {}", "error:".red().bold(), file.display(), e);
                    None
                }
            })
            .collect();
        pb.finish_and_clear();
        results
    } else {
        dirty_files
            .par_iter()
            .filter_map(|file| match processor.parse(file) {
                Ok(result) => Some(((*file).clone(), result)),
                Err(e) => {
                    eprintln!("{} {}: {}", "error:".red().bold(), file.display(), e);
                    None
                }
            })
            .collect()
    };
    let phase2_elapsed = phase2_start.elapsed();

    // Phase 2b (parallel): pre-serialize JSON fields for all parsed files.
    // serde_json::to_string is CPU-bound — rayon cuts this from O(N) serial to
    // O(N/cores) before we enter the single-threaded SQLite transaction.
    let serialized_rows: Vec<db::writer::PreserializedRow> = parse_results
        .par_iter()
        .filter_map(|(abs_path, result)| {
            let rel = abs_path
                .strip_prefix(&root)
                .unwrap_or(abs_path)
                .display()
                .to_string();
            let mtime = db::writer::file_mtime_rfc3339(abs_path);
            match db::writer::serialize_file_data(&rel, result, mtime.as_deref()) {
                Ok(row) => Some(row),
                Err(e) => {
                    eprintln!(
                        "{} serialize {}: {}",
                        "error:".red().bold(),
                        abs_path.display(),
                        e
                    );
                    None
                }
            }
        })
        .collect();

    // Phase 3 (transacted): write pre-serialized rows to DB in one commit.
    // JSON serialization already done in parallel — this loop is pure SQLite I/O.
    let phase3_start = Instant::now();
    {
        let tx = conn.transaction()?;
        if show_progress {
            let pb = ProgressBar::new(serialized_rows.len() as u64);
            pb.set_style(
                ProgressStyle::with_template("Writing  {wide_bar:.green/blue} {pos}/{len}")
                    .expect("valid template"),
            );
            for row in &serialized_rows {
                db::writer::upsert_preserialized(&tx, row)?;
                pb.inc(1);
            }
            pb.finish_and_clear();
        } else {
            for row in &serialized_rows {
                db::writer::upsert_preserialized(&tx, row)?;
            }
        }
        tx.commit()?;
    }
    let phase3_elapsed = phase3_start.elapsed();

    // Phase 4: rebuild the pre-computed reverse dependency graph.
    let phase4_start = Instant::now();
    if show_progress {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::with_template("{spinner:.blue} Building dependency graph...")
                .expect("valid template"),
        );
        sp.enable_steady_tick(Duration::from_millis(80));
        db::writer::rebuild_and_write_reverse_deps(&mut conn, &root)?;
        sp.finish_and_clear();
    } else {
        db::writer::rebuild_and_write_reverse_deps(&mut conn, &root)?;
    }
    let phase4_elapsed = phase4_start.elapsed();

    db::writer::write_meta(&conn, "fmm_version", env!("CARGO_PKG_VERSION"))?;
    db::writer::write_meta(&conn, "generated_at", &Utc::now().to_rfc3339())?;

    let total_elapsed = total_start.elapsed();

    println!(
        "{} {} file(s) indexed in {:.1}s",
        "Done ✓".green().bold(),
        dirty_files.len(),
        total_elapsed.as_secs_f64()
    );

    if !quiet {
        let accounted = phase1_elapsed + phase2_elapsed + phase3_elapsed + phase4_elapsed;
        let other = total_elapsed.saturating_sub(accounted);
        println!(
            "  parse: {:.1}s · write: {:.1}s · deps: {:.1}s · other: {:.1}s",
            phase2_elapsed.as_secs_f64(),
            phase3_elapsed.as_secs_f64(),
            phase4_elapsed.as_secs_f64(),
            other.as_secs_f64(),
        );
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
