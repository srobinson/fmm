use anyhow::Result;
use colored::Colorize;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fmm_core::store::FmmStore;
use fmm_core::types::{PreserializedRow, serialize_file_data_with_fingerprint};
use fmm_store::SqliteStore;

use fmm_core::config::Config;
use fmm_core::identity::Fingerprint;
use fmm_core::resolver;

use super::{collect_files_multi, resolve_root_multi};

mod output;
mod parse;
pub(crate) mod staleness;

use output::{
    print_all_up_to_date, print_dry_run_summary, print_files_summary, print_no_supported_files,
    print_phase_timings, run_with_spinner, start_spinner,
};

/// Show progress bars when at least this many files need processing.
const PROGRESS_THRESHOLD: usize = 10;

pub fn generate(paths: &[String], dry_run: bool, force: bool, quiet: bool) -> Result<()> {
    let total_start = Instant::now();
    let config = Config::load().unwrap_or_default();

    let scan_sp = (!quiet).then(|| start_spinner("{spinner:.blue} Scanning files..."));
    let (files, skipped) = collect_files_multi(paths, &config)?;
    let root = resolve_root_multi(paths)?;
    if let Some(sp) = &scan_sp {
        sp.finish_and_clear();
    }

    if files.is_empty() {
        print_no_supported_files(&config);
        return Ok(());
    }

    if dry_run {
        print_dry_run_summary(&files, &root, force);
        return Ok(());
    }

    let store = SqliteStore::open_or_create(&root)?;
    let workspace_info = resolver::workspace::discover(&root);
    store.upsert_workspace_packages(&workspace_info.packages)?;

    // Phase 1: bulk staleness check + apply fingerprint-only refreshes.
    let scan = staleness::stale_files(&files, &root, &store, force)?;
    for refresh in &scan.fingerprint_refreshes {
        store.update_file_fingerprint(&refresh.rel_path, &refresh.fingerprint)?;
    }
    let removed_count = delete_removed_files(&store, &scan.removed_paths)?;

    if scan.dirty_files.is_empty() {
        if removed_count > 0 {
            finish_removed_only_update(
                &store,
                &root,
                total_start,
                scan.elapsed,
                quiet,
                removed_count,
            )?;
        } else {
            print_all_up_to_date(files.len() + skipped, skipped, total_start.elapsed());
            store.write_meta()?;
        }
        return Ok(());
    }

    let show_progress = !quiet && scan.dirty_files.len() >= PROGRESS_THRESHOLD;
    if !quiet {
        print_files_summary(files.len() + skipped, skipped, scan.dirty_files.len());
    }

    // Phase 2 (parallel): parse all stale files. map_init creates one
    // ParserCache per rayon worker thread to amortize parser construction.
    let dirty_paths: Vec<&PathBuf> = scan.dirty_files.iter().map(|file| file.path).collect();
    let fingerprints_by_path: std::collections::HashMap<_, _> = scan
        .dirty_files
        .iter()
        .map(|file| (file.path.as_path(), file.fingerprint.clone()))
        .collect();
    let (parse_results, phase2_elapsed) = parse::parse_dirty_files(&dirty_paths, show_progress);

    // Phase 2b (parallel): pre-serialize JSON fields before the single-threaded
    // SQLite write phase.
    let (serialized_rows, phase2b_elapsed) =
        serialize_dirty_rows(&parse_results, &root, &fingerprints_by_path);

    // Phase 3 (transacted): write pre-serialized rows to DB in one commit.
    let is_full_generate = force || scan.dirty_files.len() == files.len();
    let phase3_start = Instant::now();
    run_with_spinner(show_progress, "{spinner:.green} Writing index...", || {
        store.write_indexed_files(&serialized_rows, is_full_generate)
    })?;
    let phase3_elapsed = phase3_start.elapsed();

    // Phase 4: rebuild the pre-computed reverse dependency graph.
    let phase4_start = Instant::now();
    run_with_spinner(
        show_progress,
        "{spinner:.blue} Building dependency graph...",
        || store.rebuild_and_write_reverse_deps(&root),
    )?;
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
        print_phase_timings(
            total_elapsed,
            scan.elapsed,
            phase2_elapsed,
            phase2b_elapsed,
            phase3_elapsed,
            phase4_elapsed,
        );
    }

    Ok(())
}

fn delete_removed_files(store: &SqliteStore, removed_paths: &[String]) -> Result<usize> {
    let mut removed_count = 0;
    for rel_path in removed_paths {
        if store.delete_single_file(rel_path)? {
            removed_count += 1;
        }
    }
    Ok(removed_count)
}

fn finish_removed_only_update(
    store: &SqliteStore,
    root: &Path,
    total_start: Instant,
    phase1_elapsed: Duration,
    quiet: bool,
    removed_count: usize,
) -> Result<()> {
    if !quiet {
        println!("Found {removed_count} removed file(s)");
    }

    let phase4_start = Instant::now();
    run_with_spinner(
        false,
        "{spinner:.blue} Building dependency graph...",
        || store.rebuild_and_write_reverse_deps(root),
    )?;
    let phase4_elapsed = phase4_start.elapsed();

    store.write_meta()?;
    let total_elapsed = total_start.elapsed();
    println!(
        "{} {} file(s) pruned in {:.1}s",
        "Done ✓".green().bold(),
        removed_count,
        total_elapsed.as_secs_f64()
    );

    if !quiet {
        print_phase_timings(
            total_elapsed,
            phase1_elapsed,
            Duration::default(),
            Duration::default(),
            Duration::default(),
            phase4_elapsed,
        );
    }

    Ok(())
}

fn serialize_dirty_rows(
    parse_results: &[(PathBuf, fmm_core::parser::ParseResult)],
    root: &Path,
    fingerprints_by_path: &std::collections::HashMap<&Path, Option<Fingerprint>>,
) -> (Vec<PreserializedRow>, Duration) {
    let start = Instant::now();
    let rows = parse_results
        .par_iter()
        .filter_map(|(abs_path, result)| {
            let rel = abs_path
                .strip_prefix(root)
                .unwrap_or(abs_path)
                .display()
                .to_string();
            let fingerprint = fingerprints_by_path
                .get(abs_path.as_path())
                .cloned()
                .flatten()
                .or_else(|| staleness::source_fingerprint(abs_path).ok());
            let Some(fingerprint) = fingerprint else {
                eprintln!(
                    "{} fingerprint {}",
                    "error:".red().bold(),
                    abs_path.display(),
                );
                return None;
            };
            match serialize_file_data_with_fingerprint(&rel, result, &fingerprint) {
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
    (rows, start.elapsed())
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
    // Load all fingerprints once to avoid per-file queries and to determine
    // the reason a file is invalid ("stale" vs "not indexed").
    let indexed_fingerprints = store.load_fingerprints()?;

    println!("Validating {} files against index...", files.len());

    let invalid: Vec<_> = files
        .iter()
        .filter_map(|file| {
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let decision = staleness::decide_file(file, &root, &indexed_fingerprints, false);
            if matches!(
                decision,
                Ok(staleness::StalenessDecision::UpToDate)
                    | Ok(staleness::StalenessDecision::RefreshFingerprint(_))
            ) {
                None
            } else {
                let reason = if indexed_fingerprints.contains_key(&rel) {
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
