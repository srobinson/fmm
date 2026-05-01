use anyhow::Result;
use colored::Colorize;
use fmm_core::config::Config;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::staleness;

pub(super) fn start_spinner(message: &str) -> ProgressBar {
    let sp = ProgressBar::new_spinner();
    sp.set_style(ProgressStyle::with_template(message).expect("valid template"));
    sp.enable_steady_tick(Duration::from_millis(80));
    sp
}

pub(super) fn run_with_spinner<T>(show: bool, message: &str, op: impl FnOnce() -> T) -> T {
    if !show {
        return op();
    }
    let sp = start_spinner(message);
    let result = op();
    sp.finish_and_clear();
    result
}

pub(super) fn print_no_supported_files(config: &Config) {
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
}

pub(super) fn print_dry_run_summary(files: &[PathBuf], root: &Path, force: bool) -> Result<()> {
    let dirty_files = staleness::dry_run_dirty_files(files, root, force)?;

    for abs_path in &dirty_files {
        let rel = abs_path.strip_prefix(root).unwrap_or(abs_path);
        println!("  {} Would index: {}", "✓".green(), rel.display());
    }
    if dirty_files.is_empty() {
        println!("{} All files up to date", "✓".green());
    } else {
        println!(
            "\n{} {} file(s) would be indexed",
            "✓".green().bold(),
            dirty_files.len()
        );
    }
    println!("\n{} (dry run — nothing written)", "!".yellow());
    Ok(())
}

pub(super) fn print_all_up_to_date(total: usize, skipped: usize, elapsed: Duration) {
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
}

pub(super) fn print_files_summary(total: usize, skipped: usize, dirty: usize) {
    if skipped > 0 {
        println!("Found {total} files · {skipped} skipped · {dirty} changed");
    } else {
        println!("Found {total} files · {dirty} changed");
    }
}

pub(super) fn print_phase_timings(
    total: Duration,
    phase1: Duration,
    phase2: Duration,
    phase2b: Duration,
    phase3: Duration,
    phase4: Duration,
) {
    let accounted = phase1 + phase2 + phase2b + phase3 + phase4;
    let other = total.saturating_sub(accounted);
    println!(
        "  parse: {:.1}s · serialize: {:.1}s · write: {:.1}s · deps: {:.1}s · other: {:.1}s",
        phase2.as_secs_f64(),
        phase2b.as_secs_f64(),
        phase3.as_secs_f64(),
        phase4.as_secs_f64(),
        other.as_secs_f64(),
    );
}
