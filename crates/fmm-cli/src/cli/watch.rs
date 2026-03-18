use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use colored::Colorize;
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};

use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_core::types::serialize_file_data;
use fmm_store::SqliteStore;

use crate::fs_utils;
use fmm_core::config::Config;

use super::{collect_files, resolve_root};

pub fn watch(path: &str, debounce_ms: u64) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let root = resolve_root(path)?;
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| root.clone());

    // Initial generate pass
    println!("{}", "Running initial generate pass...".green().bold());
    super::generate(&[path.to_string()], false, false, false)?;

    let file_count = collect_files(path, &config)?.0.len();
    println!("\nWatching {} files in {} ...\n", file_count, path);

    let updates = Arc::new(AtomicUsize::new(0));

    // Ctrl+C: print summary and exit
    let updates_for_ctrlc = updates.clone();
    ctrlc::set_handler(move || {
        let count = updates_for_ctrlc.load(Ordering::Relaxed);
        eprintln!(
            "\n{} Stopped watching. {} file(s) re-indexed.",
            "✓".green().bold(),
            count,
        );
        std::process::exit(0);
    })?;

    let updates_for_handler = updates.clone();
    let config_for_handler = config.clone();
    let root_for_handler = Arc::new(root.clone());

    // The debouncer callback runs on its own thread — we use a parking channel
    // to keep the main thread alive until Ctrl+C fires.
    let (_tx, rx) = std::sync::mpsc::channel::<()>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    for event_path in &event.paths {
                        handle_event(
                            event_path,
                            &event.kind,
                            &config_for_handler,
                            &root_for_handler,
                            &updates_for_handler,
                        );
                    }
                }
            }
        },
    )?;

    debouncer.watch(&target, RecursiveMode::Recursive)?;

    // Block forever — Ctrl+C handler exits the process
    let _ = rx.recv();

    Ok(())
}

/// Returns true if a path should be processed by the watcher.
fn is_watchable(path: &Path, config: &Config) -> bool {
    // Skip SQLite database files
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == fmm_store::DB_FILENAME || n.ends_with("-wal") || n.ends_with("-shm"))
    {
        return false;
    }
    let is_supported = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| config.is_supported_language(ext));
    if !is_supported {
        return false;
    }
    !path.is_dir()
}

fn handle_event(
    path: &Path,
    kind: &notify::EventKind,
    config: &Config,
    root: &std::path::PathBuf,
    updates: &AtomicUsize,
) {
    if !is_watchable(path, config) {
        return;
    }

    let rel = path.strip_prefix(root).unwrap_or(path);
    let display = rel.display();

    use notify::EventKind::*;
    match kind {
        Create(_) | Modify(_) => {
            if !path.exists() {
                return;
            }
            match index_file(path, root) {
                Ok(true) => {
                    println!("  {} Re-indexed {}", "✓".green(), display);
                    updates.fetch_add(1, Ordering::Relaxed);
                }
                Ok(false) => {} // unchanged
                Err(e) => {
                    eprintln!("  {} {}: {}", "✗".red(), display, e);
                }
            }
        }
        Remove(_) => match remove_file_from_db(path, root) {
            Ok(true) => {
                println!("  {} Removed {} from index", "✓".green(), display);
                updates.fetch_add(1, Ordering::Relaxed);
            }
            Ok(false) => {} // not in DB
            Err(e) => {
                eprintln!("  {} Remove {}: {}", "✗".red(), display, e);
            }
        },
        _ => {}
    }
}

/// Re-index a single file via `FmmStore`. Returns true if the store was updated.
fn index_file(path: &Path, root: &std::path::PathBuf) -> anyhow::Result<bool> {
    use fmm_core::extractor::FileProcessor;

    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    let mtime = fs_utils::file_mtime_rfc3339(path);
    let store = SqliteStore::open_or_create(root)?;

    if store.is_file_up_to_date(&rel, mtime.as_deref()) {
        return Ok(false);
    }

    let processor = FileProcessor::new(root);
    let result = processor.parse(path)?;
    let row = serialize_file_data(&rel, &result, mtime.as_deref())?;
    store.upsert_single_file(&row)?;

    // SqliteStore re-reads files from DB; manifest param is unused.
    store.rebuild_and_write_reverse_deps(&Manifest::new(), root)?;
    Ok(true)
}

/// Remove a file's store entry. Returns true if a row was deleted.
fn remove_file_from_db(path: &Path, root: &std::path::PathBuf) -> anyhow::Result<bool> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    // Only act if the DB exists (watcher may fire before generate runs)
    let db_path = root.join(fmm_store::DB_FILENAME);
    if !db_path.exists() {
        return Ok(false);
    }

    let store = SqliteStore::open(root)?;
    Ok(store.delete_single_file(&rel)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::fs;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    fn setup_watch_project() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("app.ts"),
            "export function main() {}\nexport const VERSION = 1;\n",
        )
        .unwrap();
        (tmp, Config::default())
    }

    #[test]
    fn is_watchable_accepts_supported_source_files() {
        let tmp = TempDir::new().unwrap();
        let ts_file = tmp.path().join("foo.ts");
        fs::write(&ts_file, "").unwrap();
        let config = Config::default();
        assert!(is_watchable(&ts_file, &config));
    }

    #[test]
    fn is_watchable_rejects_db_filename() {
        let tmp = TempDir::new().unwrap();
        let db_file = tmp.path().join(fmm_store::DB_FILENAME);
        fs::write(&db_file, "").unwrap();
        let config = Config::default();
        assert!(!is_watchable(&db_file, &config));
    }

    #[test]
    fn is_watchable_rejects_unsupported_extensions() {
        let tmp = TempDir::new().unwrap();
        let txt_file = tmp.path().join("readme.txt");
        fs::write(&txt_file, "").unwrap();
        let config = Config::default();
        assert!(!is_watchable(&txt_file, &config));
    }

    #[test]
    fn is_watchable_rejects_directories() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("src");
        fs::create_dir_all(&dir).unwrap();
        let config = Config::default();
        assert!(!is_watchable(&dir, &config));
    }

    #[test]
    fn is_watchable_rejects_no_extension() {
        let tmp = TempDir::new().unwrap();
        let no_ext = tmp.path().join("Makefile");
        fs::write(&no_ext, "").unwrap();
        let config = Config::default();
        assert!(!is_watchable(&no_ext, &config));
    }

    #[test]
    fn handle_create_indexes_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let updates = AtomicUsize::new(0);

        handle_event(
            &source,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );

        let store = SqliteStore::open(&root).unwrap();
        let manifest = store.load_manifest().unwrap();
        assert!(manifest.files.contains_key("src/app.ts"));
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_modify_reindexes_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let updates = AtomicUsize::new(0);

        // Index initial state
        handle_event(
            &source,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );

        // Modify source and give it a different mtime
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(
            &source,
            "export function main() {}\nexport function newFunc() {}\n",
        )
        .unwrap();

        handle_event(
            &source,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &root,
            &updates,
        );

        let store = SqliteStore::open(&root).unwrap();
        let manifest = store.load_manifest().unwrap();
        assert!(manifest.export_index.contains_key("newFunc"));
        assert_eq!(updates.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn handle_modify_skips_unchanged_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let updates = AtomicUsize::new(0);

        // Index the file
        handle_event(
            &source,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );
        assert_eq!(updates.load(Ordering::Relaxed), 1);

        // Trigger modify without touching the file — mtime unchanged, no re-index
        handle_event(
            &source,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_remove_removes_from_db() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let updates = AtomicUsize::new(0);

        // Index the file first
        handle_event(
            &source,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );
        {
            let store = SqliteStore::open(&root).unwrap();
            let manifest = store.load_manifest().unwrap();
            assert!(manifest.files.contains_key("src/app.ts"));
        }

        // Delete source and trigger remove event
        fs::remove_file(&source).unwrap();
        handle_event(
            &source,
            &notify::EventKind::Remove(RemoveKind::File),
            &config,
            &root,
            &updates,
        );

        let store = SqliteStore::open(&root).unwrap();
        let manifest = store.load_manifest().unwrap();
        assert!(!manifest.files.contains_key("src/app.ts"));
        assert_eq!(updates.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn handle_remove_noop_when_not_indexed() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let updates = AtomicUsize::new(0);

        // No DB — remove should be a no-op
        fs::remove_file(&source).unwrap();

        handle_event(
            &source,
            &notify::EventKind::Remove(RemoveKind::File),
            &config,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_event_ignores_db_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let db_file = root.join(fmm_store::DB_FILENAME);
        fs::write(&db_file, "not a real db").unwrap();
        let updates = AtomicUsize::new(0);

        handle_event(
            &db_file,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_event_ignores_unsupported_extensions() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let txt_file = root.join("notes.txt");
        fs::write(&txt_file, "some notes").unwrap();
        let updates = AtomicUsize::new(0);

        handle_event(
            &txt_file,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_create_new_file_indexes_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let new_file = root.join("src/new-component.ts");
        fs::write(&new_file, "export class Widget {}\n").unwrap();
        let updates = AtomicUsize::new(0);

        handle_event(
            &new_file,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &root,
            &updates,
        );

        let store = SqliteStore::open(&root).unwrap();
        let manifest = store.load_manifest().unwrap();
        assert!(manifest.export_index.contains_key("Widget"));
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }
}
