//! In-memory `FmmStore` implementation for testing.
//!
//! `InMemoryStore` is a lightweight, `Send`-safe store that holds all index
//! data in memory. It replicates the SQLite round-trip behavior: JSON fields
//! are deserialized at write time and re-serialized on load, so callers see
//! identical `Manifest` structures regardless of backend.
//!
//! Gated behind the `test-support` feature. Not intended for production use.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fmm_core::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};
use fmm_core::store::FmmStore;
use fmm_core::types::PreserializedRow;

/// Error type for in-memory store operations.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MemoryStoreError(String);

impl MemoryStoreError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

/// Stored representation of a single file's index data.
struct StoredFile {
    loc: i64,
    mtime: Option<String>,
    imports: Vec<String>,
    dependencies: Vec<String>,
    named_imports: HashMap<String, Vec<String>>,
    namespace_imports: Vec<String>,
    function_names: Vec<String>,
    exports: Vec<StoredExport>,
    methods: Vec<StoredMethod>,
}

struct StoredExport {
    name: String,
    start_line: i64,
    end_line: i64,
}

struct StoredMethod {
    dotted_name: String,
    start_line: i64,
    end_line: i64,
    kind: Option<String>,
}

struct InnerState {
    files: HashMap<String, StoredFile>,
    reverse_deps: HashMap<String, Vec<String>>,
    workspace_packages: HashMap<String, PathBuf>,
}

/// In-memory `FmmStore` for testing MCP tool handlers without SQLite.
///
/// Uses `Mutex` for interior mutability, making this `Send + Sync`.
/// All trait methods acquire the lock briefly; no long-held borrows.
pub struct InMemoryStore {
    state: Mutex<InnerState>,
}

impl InMemoryStore {
    /// Create an empty in-memory store.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InnerState {
                files: HashMap::new(),
                reverse_deps: HashMap::new(),
                workspace_packages: HashMap::new(),
            }),
        }
    }

    /// Ingest a `PreserializedRow`, deserializing JSON fields into structured data.
    fn ingest_row(row: &PreserializedRow) -> Result<(String, StoredFile), MemoryStoreError> {
        let imports: Vec<String> = serde_json::from_str(&row.imports_json)
            .map_err(|e| MemoryStoreError::new(format!("imports: {e}")))?;
        let dependencies: Vec<String> = serde_json::from_str(&row.deps_json)
            .map_err(|e| MemoryStoreError::new(format!("deps: {e}")))?;
        let named_imports: HashMap<String, Vec<String>> =
            serde_json::from_str(&row.named_imports_json)
                .map_err(|e| MemoryStoreError::new(format!("named_imports: {e}")))?;
        let namespace_imports: Vec<String> = serde_json::from_str(&row.namespace_imports_json)
            .map_err(|e| MemoryStoreError::new(format!("namespace_imports: {e}")))?;
        let function_names: Vec<String> = serde_json::from_str(&row.function_names_json)
            .map_err(|e| MemoryStoreError::new(format!("function_names: {e}")))?;

        let exports = row
            .exports
            .iter()
            .map(|e| StoredExport {
                name: e.name.clone(),
                start_line: e.start_line,
                end_line: e.end_line,
            })
            .collect();

        let methods = row
            .methods
            .iter()
            .map(|m| StoredMethod {
                dotted_name: m.dotted_name.clone(),
                start_line: m.start_line,
                end_line: m.end_line,
                kind: m.kind.clone(),
            })
            .collect();

        let stored = StoredFile {
            loc: row.loc,
            mtime: row.mtime.clone(),
            imports,
            dependencies,
            named_imports,
            namespace_imports,
            function_names,
            exports,
            methods,
        };

        Ok((row.rel_path.clone(), stored))
    }

    /// Build a `Manifest` from stored data, replicating the reader's index logic.
    fn build_manifest(state: &InnerState) -> Manifest {
        let mut manifest = Manifest::new();

        // Phase 1: populate file entries (without exports, filled in phase 2)
        for (path, sf) in &state.files {
            manifest.files.insert(
                path.clone(),
                FileEntry {
                    exports: Vec::new(),
                    export_lines: None,
                    methods: None,
                    imports: sf.imports.clone(),
                    dependencies: sf.dependencies.clone(),
                    loc: sf.loc as usize,
                    modified: sf.mtime.clone(),
                    function_names: sf.function_names.clone(),
                    named_imports: sf.named_imports.clone(),
                    namespace_imports: sf.namespace_imports.clone(),
                    ..Default::default()
                },
            );
        }

        // Phase 2: populate exports and global indexes
        for (file_path, sf) in &state.files {
            let mut names: Vec<String> = Vec::with_capacity(sf.exports.len());
            let mut line_ranges: Vec<ExportLines> = Vec::with_capacity(sf.exports.len());
            let mut has_lines = false;

            for exp in &sf.exports {
                names.push(exp.name.clone());
                let el = if exp.start_line > 0 {
                    has_lines = true;
                    ExportLines {
                        start: exp.start_line as usize,
                        end: exp.end_line as usize,
                    }
                } else {
                    ExportLines { start: 0, end: 0 }
                };
                line_ranges.push(el);
            }

            if let Some(entry) = manifest.files.get_mut(file_path) {
                entry.exports = names.clone();
                if has_lines {
                    entry.export_lines = Some(line_ranges.clone());
                }
            }

            // Build global indexes with TS > JS collision resolution
            for (i, exp) in sf.exports.iter().enumerate() {
                let line_range = if has_lines {
                    line_ranges
                        .get(i)
                        .and_then(|l| if l.start > 0 { Some(l.clone()) } else { None })
                } else {
                    None
                };

                // export_all: track every definition
                manifest
                    .export_all
                    .entry(exp.name.clone())
                    .or_default()
                    .push(ExportLocation {
                        file: file_path.clone(),
                        lines: line_range.clone(),
                    });

                // function_index: first definition wins for known functions
                if let Some(fe) = manifest.files.get(file_path)
                    && fe.function_names.contains(&exp.name)
                {
                    manifest
                        .function_index
                        .entry(exp.name.clone())
                        .or_insert(ExportLocation {
                            file: file_path.clone(),
                            lines: line_range.clone(),
                        });
                }

                // export_index / export_locations: TS > JS collision logic
                let should_insert = match manifest.export_index.get(&exp.name) {
                    None => true,
                    Some(existing) if existing == file_path => true,
                    Some(existing) => {
                        // .ts/.tsx takes priority: only skip if existing is TS and new is JS
                        let existing_is_ts =
                            existing.ends_with(".ts") || existing.ends_with(".tsx");
                        let new_is_js = file_path.ends_with(".js") || file_path.ends_with(".jsx");
                        !(existing_is_ts && new_is_js)
                    }
                };

                if should_insert {
                    manifest
                        .export_index
                        .insert(exp.name.clone(), file_path.clone());
                    manifest.export_locations.insert(
                        exp.name.clone(),
                        ExportLocation {
                            file: file_path.clone(),
                            lines: line_range,
                        },
                    );
                }
            }
        }

        // Phase 3: populate methods
        for (file_path, sf) in &state.files {
            for method in &sf.methods {
                let lines = if method.start_line > 0 {
                    Some(ExportLines {
                        start: method.start_line as usize,
                        end: method.end_line as usize,
                    })
                } else {
                    None
                };

                let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });

                if let Some(fe) = manifest.files.get_mut(file_path) {
                    match method.kind.as_deref() {
                        Some("nested-fn") => {
                            fe.nested_fns.insert(method.dotted_name.clone(), el);
                        }
                        Some("closure-state") => {
                            fe.closure_state.insert(method.dotted_name.clone(), el);
                        }
                        _ => {
                            fe.methods
                                .get_or_insert_with(HashMap::new)
                                .insert(method.dotted_name.clone(), el);
                        }
                    }
                }

                manifest.method_index.insert(
                    method.dotted_name.clone(),
                    ExportLocation {
                        file: file_path.clone(),
                        lines,
                    },
                );
            }
        }

        // Phase 4: reverse deps
        manifest.reverse_deps = state.reverse_deps.clone();

        // Phase 5: workspace packages
        for (name, path) in &state.workspace_packages {
            manifest.workspace_roots.push(path.clone());
            manifest
                .workspace_packages
                .insert(name.clone(), path.clone());
        }

        manifest
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FmmStore for InMemoryStore {
    type Error = MemoryStoreError;

    fn load_manifest(&self) -> Result<Manifest, Self::Error> {
        let state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        if state.files.is_empty() {
            return Err(MemoryStoreError::new("empty store: no files indexed"));
        }

        Ok(Self::build_manifest(&state))
    }

    fn load_indexed_mtimes(&self) -> Result<HashMap<String, String>, Self::Error> {
        let state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        let mtimes = state
            .files
            .iter()
            .filter_map(|(path, sf)| sf.mtime.as_ref().map(|m| (path.clone(), m.clone())))
            .collect();

        Ok(mtimes)
    }

    fn write_indexed_files(
        &self,
        rows: &[PreserializedRow],
        full_reindex: bool,
    ) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        if full_reindex {
            state.files.clear();
        }

        for row in rows {
            let (path, stored) = Self::ingest_row(row)?;
            state.files.insert(path, stored);
        }

        Ok(())
    }

    fn upsert_single_file(&self, row: &PreserializedRow) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        let (path, stored) = Self::ingest_row(row)?;
        state.files.insert(path, stored);
        Ok(())
    }

    fn delete_single_file(&self, rel_path: &str) -> Result<bool, Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        Ok(state.files.remove(rel_path).is_some())
    }

    fn rebuild_and_write_reverse_deps(
        &self,
        _manifest: &Manifest,
        _root: &Path,
    ) -> Result<(), Self::Error> {
        // For testing, reverse deps are a no-op. Tests that need reverse deps
        // should use SqliteStore, which delegates to the full resolver pipeline.
        Ok(())
    }

    fn upsert_workspace_packages(
        &self,
        packages: &HashMap<String, PathBuf>,
    ) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        for (name, path) in packages {
            state.workspace_packages.insert(name.clone(), path.clone());
        }

        Ok(())
    }

    fn write_meta(&self) -> Result<(), Self::Error> {
        // No-op for in-memory store. Meta is only meaningful for on-disk persistence.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
    use fmm_core::types::serialize_file_data;

    fn make_parse_result(exports: Vec<ExportEntry>) -> ParseResult {
        ParseResult {
            metadata: Metadata {
                exports,
                imports: vec!["react".into()],
                dependencies: vec!["./utils".into()],
                loc: 15,
                ..Default::default()
            },
            custom_fields: None,
        }
    }

    #[test]
    fn write_and_load_manifest() {
        let store = InMemoryStore::new();

        let result = make_parse_result(vec![
            ExportEntry::new("Alpha".into(), 1, 10),
            ExportEntry::new("Beta".into(), 12, 20),
        ]);
        let row =
            serialize_file_data("src/mod.ts", &result, Some("2026-01-01T00:00:00+00:00")).unwrap();

        store.write_indexed_files(&[row], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        let entry = manifest.files.get("src/mod.ts").unwrap();
        assert_eq!(entry.loc, 15);
        assert!(entry.exports.contains(&"Alpha".to_string()));
        assert!(entry.exports.contains(&"Beta".to_string()));
        assert_eq!(
            manifest.export_index.get("Alpha").map(String::as_str),
            Some("src/mod.ts")
        );
    }

    #[test]
    fn batch_write_is_atomic() {
        let store = InMemoryStore::new();

        let r1 = make_parse_result(vec![ExportEntry::new("A".into(), 1, 5)]);
        let r2 = make_parse_result(vec![ExportEntry::new("B".into(), 1, 5)]);
        let row1 = serialize_file_data("src/a.ts", &r1, None).unwrap();
        let row2 = serialize_file_data("src/b.ts", &r2, None).unwrap();

        store.write_indexed_files(&[row1, row2], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(manifest.files.contains_key("src/a.ts"));
        assert!(manifest.files.contains_key("src/b.ts"));
    }

    #[test]
    fn full_reindex_clears_old_data() {
        let store = InMemoryStore::new();

        let r1 = make_parse_result(vec![ExportEntry::new("Old".into(), 1, 5)]);
        let row1 = serialize_file_data("src/old.ts", &r1, None).unwrap();
        store.write_indexed_files(&[row1], true).unwrap();

        let r2 = make_parse_result(vec![ExportEntry::new("New".into(), 1, 5)]);
        let row2 = serialize_file_data("src/new.ts", &r2, None).unwrap();
        store.write_indexed_files(&[row2], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(!manifest.files.contains_key("src/old.ts"));
        assert!(manifest.files.contains_key("src/new.ts"));
    }

    #[test]
    fn upsert_single_file() {
        let store = InMemoryStore::new();

        let result = make_parse_result(vec![ExportEntry::new("Foo".into(), 1, 5)]);
        let row = serialize_file_data("src/foo.ts", &result, None).unwrap();

        store.upsert_single_file(&row).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(manifest.files.contains_key("src/foo.ts"));
    }

    #[test]
    fn delete_single_file() {
        let store = InMemoryStore::new();

        let result = make_parse_result(vec![ExportEntry::new("Bar".into(), 1, 5)]);
        let row = serialize_file_data("src/bar.ts", &result, None).unwrap();
        store.upsert_single_file(&row).unwrap();

        let deleted = store.delete_single_file("src/bar.ts").unwrap();
        assert!(deleted);

        let not_found = store.delete_single_file("src/bar.ts").unwrap();
        assert!(!not_found);
    }

    #[test]
    fn load_indexed_mtimes() {
        let store = InMemoryStore::new();

        let result = make_parse_result(vec![]);
        let row =
            serialize_file_data("src/x.ts", &result, Some("2026-03-01T00:00:00+00:00")).unwrap();
        store.upsert_single_file(&row).unwrap();

        let mtimes = store.load_indexed_mtimes().unwrap();
        assert!(mtimes.contains_key("src/x.ts"));
    }

    #[test]
    fn empty_store_returns_error() {
        let store = InMemoryStore::new();
        assert!(store.load_manifest().is_err());
    }

    #[test]
    fn is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<InMemoryStore>();
    }

    #[test]
    fn ts_wins_over_js_collision() {
        let store = InMemoryStore::new();

        let js = make_parse_result(vec![ExportEntry::new("Widget".into(), 1, 5)]);
        let ts = make_parse_result(vec![ExportEntry::new("Widget".into(), 1, 5)]);
        let row_js = serialize_file_data("src/widget.js", &js, None).unwrap();
        let row_ts = serialize_file_data("src/widget.ts", &ts, None).unwrap();

        store.write_indexed_files(&[row_js, row_ts], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert_eq!(
            manifest.export_index.get("Widget").map(String::as_str),
            Some("src/widget.ts")
        );
        assert_eq!(manifest.export_all.get("Widget").unwrap().len(), 2);
    }

    #[test]
    fn methods_loaded_into_method_index() {
        let store = InMemoryStore::new();

        let result = make_parse_result(vec![ExportEntry::method(
            "run".into(),
            5,
            15,
            "Server".into(),
        )]);
        let row = serialize_file_data("src/server.ts", &result, None).unwrap();

        store.write_indexed_files(&[row], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        let loc = manifest.method_index.get("Server.run").unwrap();
        assert_eq!(loc.file, "src/server.ts");
        assert_eq!(loc.lines.as_ref().unwrap().start, 5);
    }

    #[test]
    fn workspace_packages() {
        let store = InMemoryStore::new();

        // Need at least one file to load manifest
        let result = make_parse_result(vec![]);
        let row = serialize_file_data("src/lib.ts", &result, None).unwrap();
        store.upsert_single_file(&row).unwrap();

        let mut pkgs = HashMap::new();
        pkgs.insert("core".to_string(), PathBuf::from("/repo/packages/core"));
        store.upsert_workspace_packages(&pkgs).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(manifest.workspace_packages.contains_key("core"));
    }
}
