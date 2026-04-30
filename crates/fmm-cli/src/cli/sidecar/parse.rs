use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fmm_core::extractor::ParserCache;
use fmm_core::parser::ParseResult;

pub(crate) fn parse_dirty_files(
    dirty_files: &[&PathBuf],
    show_progress: bool,
) -> (Vec<(PathBuf, ParseResult)>, Duration) {
    let start = Instant::now();
    let parse_results = if show_progress {
        parse_with_progress(dirty_files)
    } else {
        parse_without_progress(dirty_files)
    };
    (parse_results, start.elapsed())
}

fn parse_with_progress(dirty_files: &[&PathBuf]) -> Vec<(PathBuf, ParseResult)> {
    let pb = ProgressBar::new(dirty_files.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "Parsing  {wide_bar:.cyan/blue} {pos}/{len}  {per_sec}  {msg}",
        )
        .expect("valid template"),
    );
    pb.set_message("starting...");
    pb.enable_steady_tick(Duration::from_millis(100));

    let last_inc_ms = Arc::new(AtomicU64::new(now_ms()));
    let pb_w = pb.clone();
    let last_inc_w = Arc::clone(&last_inc_ms);
    let watcher = std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let pos = pb_w.position();
            let len = pb_w.length().unwrap_or(pos);
            if pos >= len {
                break;
            }
            let stall_secs = now_ms().saturating_sub(last_inc_w.load(Ordering::Relaxed)) / 1000;
            if stall_secs >= 2 {
                pb_w.set_message(format!("{} remaining  ({}s)", len - pos, stall_secs));
            } else {
                let eta = pb_w.eta();
                let secs = eta.as_secs();
                if secs > 1 {
                    let msg = if secs >= 60 {
                        format!("ETA {}m{}s", secs / 60, secs % 60)
                    } else {
                        format!("ETA {}s", secs)
                    };
                    pb_w.set_message(msg);
                } else {
                    pb_w.set_message("finishing...");
                }
            }
        }
    });

    let results = dirty_files
        .iter()
        .par_bridge()
        .map_init(ParserCache::new, |cache, file| {
            let r = parse_one(cache, file);
            last_inc_ms.store(now_ms(), Ordering::Relaxed);
            pb.inc(1);
            r
        })
        .filter_map(|x| x)
        .collect();
    pb.finish_and_clear();
    let _ = watcher.join();
    results
}

fn parse_without_progress(dirty_files: &[&PathBuf]) -> Vec<(PathBuf, ParseResult)> {
    dirty_files
        .iter()
        .par_bridge()
        .map_init(ParserCache::new, parse_one)
        .filter_map(|x| x)
        .collect()
}

fn parse_one(cache: &mut ParserCache, file: &&PathBuf) -> Option<(PathBuf, ParseResult)> {
    match cache.parse_file(file) {
        Ok(result) => Some(((*file).clone(), result)),
        Err(e) => {
            eprintln!("{} {}: {}", "error:".red().bold(), file.display(), e);
            None
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
