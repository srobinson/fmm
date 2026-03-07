# ALP-917 Handover Notes (Iter 3)

## State
- ALP-917 In Progress in Linear
- Core source changes DONE (committed 46c06a7)
- `just check` passes — lib and binary compile cleanly
- Test migration INCOMPLETE — tests compile but many will fail at runtime

## What Was Done This Iteration

All sidecar infrastructure removed:
- Deleted `src/formatter/mod.rs` and `src/manifest/sidecar_parser.rs`
- Removed `load_from_sidecars()` from Manifest
- `Manifest::load()` now calls SQLite only (no fallback)
- `generate()` is SQLite-only — no sidecar files written
- `watch.rs` rewritten to use SQLite directly
- `status.rs`, `init.rs` updated
- `serde_yaml` removed from Cargo.toml
- `yaml_escape()` moved from `formatter` → `format` module
- `workspace.rs` YAML parser replaced with line-based parser

## Tests Still Needing Migration

Run `just test` to see which tests fail. These need the full migration:

### tests/cli_integration.rs (HIGHEST PRIORITY)
All assertions use `sidecar_exists()` / `sidecar_content()` helpers.

Add these helpers at the top:
```rust
fn db_exists(base: &Path) -> bool { base.join(".fmm.db").exists() }
fn db_file_count(base: &Path) -> i64 {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0)
}
fn db_export_count(base: &Path, name: &str) -> i64 {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    conn.query_row("SELECT COUNT(*) FROM exports WHERE name = ?1",
        rusqlite::params![name], |r| r.get(0)).unwrap_or(0)
}
fn db_indexed(base: &Path, rel: &str) -> bool {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    conn.query_row("SELECT COUNT(*) FROM files WHERE path = ?1",
        rusqlite::params![rel], |r| r.get::<_,i64>(0)).unwrap_or(0) > 0
}
```

Test mapping:
- `generate_creates_sidecars` → `generate_creates_db`: assert `db_exists(tmp.path())`
- `generate_sidecar_content_is_valid_yaml` → `generate_db_has_expected_exports`:
  assert `db_export_count(tmp.path(), "validateUser") == 1`
  assert `db_export_count(tmp.path(), "AuthService") == 1`
- `generate_skips_unchanged_sidecars` → check count unchanged between two generate() calls
- `generate_updates_stale_sidecars` → assert `db_export_count(tmp.path(), "NEW_EXPORT") == 1`
- `generate_dry_run_creates_no_files` → assert `!db_exists(tmp.path())`
- `generate_dry_run_preserves_stale_sidecars` → assert `db_export_count(.., "DRY_RUN_TEST") == 0`
- `clean_removes_all_sidecars` → `clean_clears_db`: assert `db_file_count == 0`
- `clean_dry_run_preserves_files` → assert count unchanged
- `full_workflow_generate_validate_clean` → use `db_indexed()` and `db_file_count()`
- `respects_gitignore` → `db_indexed(path, "src/auth.ts")` && `!db_indexed(path, "src/utils.py")`
- `respects_fmmignore` → `db_indexed(path, "src/auth.ts")` && `!db_indexed(path, "src/db.ts")`
- `single_file_generate` → `db_indexed(path, "src/auth.ts")` && `!db_indexed(path, "src/db.ts")`

Remove helpers `sidecar_exists()` and `sidecar_content()`.

Add rusqlite to dev-dependencies if not already there (check Cargo.toml).
Actually no - just use `db_file_count` etc. via fmm::db or open directly.

### tests/mcp_tools.rs (MEDIUM PRIORITY)

Current state: `setup_mcp_server()` writes source AND sidecar files.
After migration: write source only, call generate(), then create server.

Steps:
1. Replace `write_source_and_sidecar(path, source, _sidecar)` with `write_source(path, source)`:
```rust
fn write_source(source_path: &std::path::Path, source: &str) {
    std::fs::write(source_path, source).unwrap();
}
```
2. At END of `setup_mcp_server()` before `McpServer::with_root()`:
```rust
fmm::cli::generate(&[tmp.path().to_str().unwrap().to_string()], false, false).unwrap();
```
3. Update `manifest_loads_from_db` to assert `files.len() == 5` (not 5 || 0).

The source files already contain the correct TypeScript — the parser will extract the same data the sidecars had.

### tests/cross_package_resolution.rs (MOST COMPLEX)

Steps:
1. Replace `write_sidecar(base, rel, yaml)` with `write_ts_source(base, rel, ts_content)`:
```rust
fn write_ts_source(base: &Path, rel: &str, content: &str) {
    let p = base.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}
```

2. Replace each `write_sidecar()` call with `write_ts_source()` with real TypeScript:
   - `imports: [pkg]` → `import { x } from 'pkg';\n`
   - `dependencies: [./path]` → `import { y } from './path';\n`
   - `exports: [name: [1,1]]` → `export const name = 1;\n`
   - No imports/exports → `// empty\n`

3. Change all path assertions from ABSOLUTE to RELATIVE:
   BEFORE: `root.join("packages/shared/utils.ts").to_string_lossy().to_string()`
   AFTER: `"packages/shared/utils.ts".to_string()`
   (SQLite stores relative paths; sidecar stored absolute from file: field)

4. The `load_manifest` helper already calls generate() — just remove the TODO comment.

### tests/named_import_precision.rs (EASIEST)

Already writes real TypeScript source files! Just:
1. Remove all `std::fs::write(root.join("src/*.fmm"), "...")` calls (12 writes)
2. Add before `McpServer::with_root()`:
```rust
fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false).unwrap();
```
3. The TS parser correctly extracts named_imports, function_names, namespace_imports.

## After All Tests Pass

1. Run `just check && just build && just test`
2. Delete this handover file
3. Mark ALP-917 as "Worker Done" in Linear
4. Commit: `nancy[ALP-917]: Complete test migration — all tests pass`
5. Update ISSUES.md checkbox [X] for ALP-917
6. Move on to ALP-918 (docs update)
