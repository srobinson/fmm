use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use colored::Colorize;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

use crate::config::Config;
use crate::extractor::{sidecar_path_for, FileProcessor};

use super::{collect_files, resolve_root};

pub fn watch(path: &str, debounce_ms: u64) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let root = resolve_root(path)?;
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| root.clone());

    // Initial generate pass
    println!("{}", "Running initial generate pass...".green().bold());
    super::generate(path, false)?;

    let file_count = collect_files(path, &config)?.len();
    println!("\nWatching {} files in {} ...\n", file_count, path);

    let updates = Arc::new(AtomicUsize::new(0));

    // Ctrl+C: print summary and exit
    let updates_for_ctrlc = updates.clone();
    ctrlc::set_handler(move || {
        let count = updates_for_ctrlc.load(Ordering::Relaxed);
        eprintln!(
            "\n{} Stopped watching. {} sidecar(s) updated.",
            "✓".green().bold(),
            count,
        );
        std::process::exit(0);
    })?;

    let updates_for_handler = updates.clone();
    let config_for_handler = config.clone();
    let root_for_handler = root.clone();

    // The debouncer callback runs on its own thread — we use a parking channel
    // to keep the main thread alive until Ctrl+C fires.
    let (_tx, rx) = std::sync::mpsc::channel::<()>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                let processor = FileProcessor::new(&root_for_handler);
                for event in events {
                    for event_path in &event.paths {
                        handle_event(
                            event_path,
                            &event.kind,
                            &config_for_handler,
                            &processor,
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
    if path
        .extension()
        .is_some_and(|ext| ext.to_str() == Some("fmm"))
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
    processor: &FileProcessor,
    root: &Path,
    updates: &AtomicUsize,
) {
    if !is_watchable(path, config) {
        return;
    }

    let sidecar = sidecar_path_for(path);
    let display = sidecar.strip_prefix(root).unwrap_or(&sidecar).display();

    use notify::EventKind::*;
    match kind {
        Create(_) | Modify(_) => {
            if !path.exists() {
                return;
            }
            match processor.process(path, false) {
                Ok(Some(msg)) => {
                    let verb = if msg.contains("Updated") {
                        "Updated"
                    } else {
                        "Created"
                    };
                    println!("  {} {} {}", "✓".green(), verb, display);
                    updates.fetch_add(1, Ordering::Relaxed);
                }
                Ok(None) => {} // unchanged
                Err(e) => {
                    eprintln!("  {} {}: {}", "✗".red(), display, e);
                }
            }
        }
        Remove(_) => {
            if sidecar.exists() {
                match std::fs::remove_file(&sidecar) {
                    Ok(()) => {
                        println!("  {} Removed {}", "✓".green(), display);
                        updates.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("  {} Remove {}: {}", "✗".red(), display, e);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::fs;
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
    fn is_watchable_rejects_fmm_sidecar_files() {
        let tmp = TempDir::new().unwrap();
        let fmm_file = tmp.path().join("foo.ts.fmm");
        fs::write(&fmm_file, "").unwrap();
        let config = Config::default();
        assert!(!is_watchable(&fmm_file, &config));
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
    fn handle_create_generates_sidecar() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        handle_event(
            &source,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &processor,
            &root,
            &updates,
        );

        assert!(sidecar_path_for(&source).exists());
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_modify_updates_sidecar() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        // Create initial sidecar
        processor.process(&source, false).unwrap();

        // Modify source
        fs::write(
            &source,
            "export function main() {}\nexport function newFunc() {}\n",
        )
        .unwrap();

        handle_event(
            &source,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &processor,
            &root,
            &updates,
        );

        let sidecar_content = fs::read_to_string(sidecar_path_for(&source)).unwrap();
        assert!(sidecar_content.contains("newFunc"));
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_modify_skips_unchanged_file() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        // Create initial sidecar
        processor.process(&source, false).unwrap();

        // Same content — should not increment updates
        handle_event(
            &source,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &processor,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_remove_deletes_orphaned_sidecar() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        // Create sidecar
        processor.process(&source, false).unwrap();
        let sidecar = sidecar_path_for(&source);
        assert!(sidecar.exists());

        // Delete source file
        fs::remove_file(&source).unwrap();

        handle_event(
            &source,
            &notify::EventKind::Remove(RemoveKind::File),
            &config,
            &processor,
            &root,
            &updates,
        );

        assert!(!sidecar.exists());
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_remove_noop_when_no_sidecar() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let source = root.join("src/app.ts");
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        // No sidecar exists — remove should be a no-op
        fs::remove_file(&source).unwrap();

        handle_event(
            &source,
            &notify::EventKind::Remove(RemoveKind::File),
            &config,
            &processor,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_event_ignores_fmm_files() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let fmm_file = root.join("src/app.ts.fmm");
        fs::write(&fmm_file, "file: src/app.ts\n").unwrap();
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        handle_event(
            &fmm_file,
            &notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &config,
            &processor,
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
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        handle_event(
            &txt_file,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &processor,
            &root,
            &updates,
        );

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn handle_create_new_file_generates_sidecar() {
        let (tmp, config) = setup_watch_project();
        let root = tmp.path().canonicalize().unwrap();
        let new_file = root.join("src/new-component.ts");
        fs::write(&new_file, "export class Widget {}\n").unwrap();
        let processor = FileProcessor::new(&root);
        let updates = AtomicUsize::new(0);

        handle_event(
            &new_file,
            &notify::EventKind::Create(CreateKind::File),
            &config,
            &processor,
            &root,
            &updates,
        );

        let sidecar = sidecar_path_for(&new_file);
        assert!(sidecar.exists());
        let content = fs::read_to_string(&sidecar).unwrap();
        assert!(content.contains("Widget"));
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }
}
