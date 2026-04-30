use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;

use crate::fs_utils;

pub(crate) fn dry_run_dirty_files<'a>(
    files: &'a [PathBuf],
    root: &Path,
    force: bool,
) -> Vec<&'a PathBuf> {
    match SqliteStore::open(root) {
        Ok(store) => files
            .iter()
            .filter(|file| {
                if force {
                    return true;
                }
                let rel = rel_path(file, root);
                let mtime = fs_utils::file_mtime_rfc3339(file);
                !store.is_file_up_to_date(&rel, mtime.as_deref())
            })
            .collect(),
        _ => files.iter().collect(),
    }
}

pub(crate) fn stale_files<'a>(
    files: &'a [PathBuf],
    root: &Path,
    store: &SqliteStore,
    force: bool,
) -> Result<(Vec<&'a PathBuf>, Duration)> {
    let start = Instant::now();
    let indexed_mtimes: HashMap<String, String> = if !force {
        store.load_indexed_mtimes()?
    } else {
        HashMap::new()
    };
    let dirty_files = files
        .par_iter()
        .filter(|file| is_stale(file, root, &indexed_mtimes, force))
        .collect();
    Ok((dirty_files, start.elapsed()))
}

fn is_stale(
    file: &Path,
    root: &Path,
    indexed_mtimes: &HashMap<String, String>,
    force: bool,
) -> bool {
    if force {
        return true;
    }

    let Some(mtime) = fs_utils::file_mtime_rfc3339(file) else {
        return true;
    };

    indexed_mtimes
        .get(&rel_path(file, root))
        .map(|indexed_at| indexed_at.as_str() < mtime.as_str())
        .unwrap_or(true)
}

fn rel_path(file: &Path, root: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}
