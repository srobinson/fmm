# ALP-917 Handover Notes

## State
- ALP-916 DONE (committed ad4adb8)
- ALP-917 In Progress in Linear, NO code changes yet — this is the plan

## What to Remove

### Files to DELETE entirely:
- src/formatter/mod.rs (180 LOC, YAML renderer)
- src/manifest/sidecar_parser.rs (384 LOC, YAML deserializer)

### src/manifest/mod.rs — REMOVE:
- `mod sidecar_parser;` + `use sidecar_parser::parse_sidecar;` (lines ~15-18)
- `load_from_sidecars()` method (lines ~178-309) 
- `add_file()` if only used by sidecar loading — check callers
- The WalkBuilder *.fmm pattern in load_from_sidecars
- `Manifest::load()` fallback to sidecars (src/manifest/mod.rs:329)
- `Manifest::load_from_sqlite()` rename to `Manifest::load()` for simplicity
- In `mcp/mod.rs:68`: `self.manifest = Manifest::load_from_sidecars(&self.root).ok();`
  → replace with `Manifest::load(&self.root).ok()`

### src/extractor/mod.rs — REMOVE functions:
- `sidecar_path_for()` (lines 23-27)
- `format_sidecar()` (lines 113-137)  
- `FileProcessor::process()` method
- `FileProcessor::validate()` method
- `FileProcessor::clean()` method
- `content_without_modified()` helper
- All serde_yaml + formatter imports
- Tests: test_sidecar_path_for, content_without_modified_* (delete)
- Keep: `FileProcessor::new()`, `extract_metadata()`, `parse()`, `parse_content()`

### src/cli/sidecar.rs — REMOVE:
- Legacy sidecar path in generate() (lines ~106-158 — the entire bottom half)
- The sidecar_path_for display in generate()
- Transition block in clean() (lines ~290-310, the ALP-917 comment block)
- Imports: `use crate::extractor::{sidecar_path_for, FileProcessor};`
- Update generate() output to say "indexed" not "sidecar(s) written"
- Update generate() to not use processor.process() at all

### src/cli/watch.rs — REWRITE handle_event():
- Replace FileProcessor::new() + processor.process() with direct SQLite write
- On Create/Modify: call db::open_or_create(&root), then upsert_file_data for the file,
  then rebuild_and_write_reverse_deps
- On Remove: DELETE FROM files WHERE path = ?
- is_watchable(): still filter out .fmm extension (but that's gone, change to DB_FILENAME)
- Watch tests: rewrite to check DB state instead of sidecar file existence
- Keep is_watchable, handle_event structure; just replace internals

### src/cli/status.rs — UPDATE:
- Remove: `use crate::extractor::sidecar_path_for;`
- Replace sidecar count with DB file count
  `let sidecar_count = files.iter().filter(|f| sidecar_path_for(f).exists()).count()`
  → query DB: `SELECT COUNT(*) FROM files`
  → or just: `let db_exists = root.join(DB_FILENAME).exists();`
  → simplest: show "N source files, X indexed" from DB query

### src/cli/init.rs — UPDATE:
- Lines 64-84: remove the "show sample sidecar" block (reads .fmm file)
  → replace with: show DB stats after generate (e.g., "Indexed N exports")
- Remove: `use crate::extractor::sidecar_path_for;`

### Cargo.toml — REMOVE:
- `serde_yaml = "0.9"` from [dependencies]
- Verify `ignore` crate still needed for source file discovery (YES, used in collect_files)

### src/lib.rs — CHECK:
- Remove `pub mod formatter;` if present

## Tests to Update

### tests/mcp_tools.rs (1223 lines):
- Replace `write_source_and_sidecar()` with `write_source()` (no sidecar write)
- At end of `setup_mcp_server()`, call `fmm::cli::generate(&[tmp.path()...], false, false)`
  to build the DB
- Replace `fmm::manifest::Manifest::load_from_sidecars(tmp.path())` with
  `fmm::manifest::Manifest::load(tmp.path())`
- Test `manifest_loads_from_sidecars` → rename to `manifest_loads_from_db`
- Test `export_index_consistency` → same pattern
- Remove sidecar content parameters from all `write_source_and_sidecar` calls

### tests/cross_package_resolution.rs (400 lines):
- `write_sidecar()` → `write_ts_source()` that writes a real .ts file
- `load_manifest()` → use `generate()` + `Manifest::load()`
- Source file content: replace `// source` with real TypeScript
  - A file with `imports: [pkg-name]` → `import { x } from 'pkg-name';`
  - A file with `exports: [x]` → `export const x = 1;`
  - A file with `dependencies: [./path]` → `import { y } from './path';`
- The sidecar YAML fields map to TypeScript constructs:
  - `imports: [...]` → external package imports
  - `dependencies: [...]` → relative/local imports  
  - `exports: [name: [line, line]]` → exported symbols
- workspace package.json setup stays the same

### tests/named_import_precision.rs:
- Already writes real TypeScript source files!
- Just remove the separate `.fmm` sidecar writes
- Call `fmm::cli::generate()` instead
- The TypeScript source content already has the right imports/exports for the parser
  to extract named_imports correctly

### tests/cli_integration.rs:
- Remove `sidecar_exists()` helper and `sidecar_content()` helper
- Update tests that assert on sidecar file existence:
  - `generate_creates_sidecars`: rename to `generate_creates_db`, assert DB exists
  - `generate_sidecar_content_is_valid_yaml`: delete (YAML no longer generated)
  - `generate_updates_stale_sidecars`: rename to `generate_updates_stale_files`, 
    assert DB has correct data
  - `generate_skips_unchanged_sidecars`: keep but verify via DB count or validate
  - `generate_dry_run_preserves_stale_sidecars`: rename, check DB not modified
  - All sidecar_exists assertions → DB query or validate() call

## Key Insight: watch.rs
The watch command needs a Connection per event (since Connection is not Send).
Use `db::open_or_create(&root)` in the event handler, not a shared connection.
The event handler gets `root: Arc<PathBuf>` already. Open connection per event (fast, WAL mode).

## Cargo.toml after removal
Run: cargo tree | grep serde_yaml  (should be empty)
Run: cargo check (no dead code warnings)
