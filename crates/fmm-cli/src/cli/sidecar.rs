use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::time::{Duration, Instant};

use fmm_core::store::FmmStore;
use fmm_core::types::{PreserializedRow, serialize_file_data};
use fmm_store::SqliteStore;

use crate::fs_utils;
use fmm_core::config::Config;
use fmm_core::resolver;

use super::{collect_files_multi, resolve_root_multi};

mod parse;
mod staleness;

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

    let (files, skipped) = collect_files_multi(paths, &config)?;
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
        let dirty_files = staleness::dry_run_dirty_files(&files, &root, force);

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

    // --- FmmStore write path ---
    let store = SqliteStore::open_or_create(&root)?;

    // Store workspace packages so the read path can resolve cross-package imports.
    let workspace_info = resolver::workspace::discover(&root);
    store.upsert_workspace_packages(&workspace_info.packages)?;

    // Phase 1: bulk staleness check.
    // Load all indexed_at times in one query (avoids 39k individual SELECTs),
    // then compare in parallel with rayon (mtime syscalls are I/O-parallel).
    let (dirty_files, phase1_elapsed) = staleness::stale_files(&files, &root, &store, force)?;

    if dirty_files.is_empty() {
        let elapsed = total_start.elapsed();
        let total = files.len() + skipped;
        if skipped > 0 {
            println!(
                "Found {} files · {} skipped · all up to date  ({:.1}s)",
                total,
                skipped,
                elapsed.as_secs_f64()
            );
        } else {
            println!(
                "Found {} files · all up to date  ({:.1}s)",
                total,
                elapsed.as_secs_f64()
            );
        }
        store.write_meta()?;
        return Ok(());
    }

    let show_progress = !quiet && dirty_files.len() >= PROGRESS_THRESHOLD;

    if !quiet {
        let total = files.len() + skipped;
        if skipped > 0 {
            println!(
                "Found {} files · {} skipped · {} changed",
                total,
                skipped,
                dirty_files.len()
            );
        } else {
            println!("Found {} files · {} changed", total, dirty_files.len());
        }
    }

    // Phase 2 (parallel): parse all stale files.
    // map_init creates one ParserCache per rayon worker thread — parsers and
    // compiled queries are reused across files instead of constructed per-file.
    let (parse_results, phase2_elapsed) = parse::parse_dirty_files(&dirty_files, show_progress);

    // Phase 2b (parallel): pre-serialize JSON fields for all parsed files.
    // serde_json::to_string is CPU-bound — rayon cuts this from O(N) serial to
    // O(N/cores) before we enter the single-threaded SQLite transaction.
    let phase2b_start = Instant::now();
    let serialized_rows: Vec<PreserializedRow> = parse_results
        .par_iter()
        .filter_map(|(abs_path, result)| {
            let rel = abs_path
                .strip_prefix(&root)
                .unwrap_or(abs_path)
                .display()
                .to_string();
            let mtime = fs_utils::file_mtime_rfc3339(abs_path);
            match serialize_file_data(&rel, result, mtime.as_deref()) {
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
    let phase2b_elapsed = phase2b_start.elapsed();

    // Phase 3 (transacted): write pre-serialized rows to DB in one commit.
    // FmmStore::write_indexed_files handles the full transaction internally:
    // full reindex DELETEs all files first (CASCADE), then uses plain INSERT;
    // incremental uses INSERT OR REPLACE per row.
    let is_full_generate = force || dirty_files.len() == files.len();
    let phase3_start = Instant::now();
    if show_progress {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::with_template("{spinner:.green} Writing index...")
                .expect("valid template"),
        );
        sp.enable_steady_tick(Duration::from_millis(80));
        store.write_indexed_files(&serialized_rows, is_full_generate)?;
        sp.finish_and_clear();
    } else {
        store.write_indexed_files(&serialized_rows, is_full_generate)?;
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
        store.rebuild_and_write_reverse_deps(&root)?;
        sp.finish_and_clear();
    } else {
        store.rebuild_and_write_reverse_deps(&root)?;
    }
    let phase4_elapsed = phase4_start.elapsed();

    store.write_meta()?;

    let total_elapsed = total_start.elapsed();

    println!(
        "{} {} file(s) indexed in {:.1}s",
        "Done ✓".green().bold(),
        serialized_rows.len(),
        total_elapsed.as_secs_f64()
    );

    if !quiet {
        let accounted =
            phase1_elapsed + phase2_elapsed + phase2b_elapsed + phase3_elapsed + phase4_elapsed;
        let other = total_elapsed.saturating_sub(accounted);
        println!(
            "  parse: {:.1}s · serialize: {:.1}s · write: {:.1}s · deps: {:.1}s · other: {:.1}s",
            phase2_elapsed.as_secs_f64(),
            phase2b_elapsed.as_secs_f64(),
            phase3_elapsed.as_secs_f64(),
            phase4_elapsed.as_secs_f64(),
            other.as_secs_f64(),
        );
    }

    Ok(())
}

pub fn validate(paths: &[String]) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let (files, _) = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        println!(
            "\n  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    let db_path = root.join(fmm_store::DB_FILENAME);
    if !db_path.exists() {
        println!("{} No fmm database found", "✗".red().bold());
        println!("\n  {} Run 'fmm generate' first", "fix:".cyan());
        anyhow::bail!("Validation failed: no database");
    }

    let store = SqliteStore::open(&root)?;
    // Load all indexed mtimes once to avoid per-file queries and to determine
    // the reason a file is invalid ("stale" vs "not indexed").
    let indexed_mtimes = store.load_indexed_mtimes()?;

    println!("Validating {} files against index...", files.len());

    let invalid: Vec<_> = files
        .iter()
        .filter_map(|file| {
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let mtime = fs_utils::file_mtime_rfc3339(file);
            if store.is_file_up_to_date(&rel, mtime.as_deref()) {
                None
            } else {
                let reason = if indexed_mtimes.contains_key(&rel) {
                    "stale".to_string()
                } else {
                    "not indexed".to_string()
                };
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
    let db_path = root.join(fmm_store::DB_FILENAME);
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
                println!("  Would remove: {}", fmm_store::DB_FILENAME);
            } else {
                let store = SqliteStore::open_unchecked(&root)?;
                let count = store.file_count()?;
                println!(
                    "  Would clear {} indexed file(s) from {}",
                    count,
                    fmm_store::DB_FILENAME
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
            println!("{} Removed {}", "✓".green(), fmm_store::DB_FILENAME);
        } else {
            let store = SqliteStore::open_unchecked(&root)?;
            let count = store.file_count()?;
            store.clear_index()?;
            println!(
                "{} Cleared {} file(s) from index ({})",
                "✓".green(),
                count,
                fmm_store::DB_FILENAME
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
