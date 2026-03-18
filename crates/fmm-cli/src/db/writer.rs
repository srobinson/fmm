use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, Transaction, params};
use std::collections::HashMap;
use std::path::Path;

use crate::manifest::FileEntry;
use crate::parser::ParseResult;

// Re-export domain types from fmm-core for backward compatibility.
pub use fmm_core::types::{
    ExportRecord, MethodRecord, PreserializedRow, extract_function_names, serialize_file_data,
};

/// Returns the file's last-modified time as an RFC3339 string, or `None`
/// if the metadata cannot be read.
///
/// Includes nanoseconds when the OS provides sub-second precision (APFS, Linux
/// ext4) so that same-second modifications are correctly detected by
/// `is_file_up_to_date`.
pub fn file_mtime_rfc3339(path: &Path) -> Option<String> {
    use std::time::SystemTime;
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let duration = mtime.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let dt = chrono::DateTime::<Utc>::from_timestamp(
        duration.as_secs() as i64,
        duration.subsec_nanos(),
    )?;
    Some(dt.to_rfc3339())
}

/// Returns `true` when the DB's `indexed_at` for `rel_path` is >= `source_mtime`,
/// meaning the stored data is at least as fresh as the source file.
pub fn is_file_up_to_date(conn: &Connection, rel_path: &str, source_mtime: Option<&str>) -> bool {
    let Some(mtime) = source_mtime else {
        return false;
    };
    conn.query_row(
        "SELECT indexed_at FROM files WHERE path = ?1",
        params![rel_path],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|indexed_at| indexed_at.as_str() >= mtime)
    .unwrap_or(false)
}

/// Load all `(path, indexed_at)` pairs from the DB in one query.
///
/// Used by the bulk staleness check in `fmm generate` to avoid 39k individual
/// queries. The returned map is keyed by relative file path.
pub fn load_indexed_mtimes(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT path, indexed_at FROM files")?;
    let map = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

// PreserializedRow, ExportRecord, MethodRecord, and serialize_file_data
// are re-exported from fmm_core::types above.

/// Write a pre-serialized file row to the DB within an open transaction.
///
/// Unlike `upsert_file_data`, this takes already-serialized JSON strings so
/// the CPU-bound serialization work can be done outside the transaction.
///
/// `plain_insert` — when `true`, use plain `INSERT` (caller must have deleted
/// the file rows beforehand via `delete_all_files`); when `false`, use
/// `INSERT OR REPLACE` which triggers CASCADE deletes per row. The
/// `prepare_cached` path caches the statement across the 39k-row loop.
pub fn upsert_preserialized(
    tx: &Transaction<'_>,
    row: &PreserializedRow,
    plain_insert: bool,
) -> Result<()> {
    {
        let sql = if plain_insert {
            "INSERT INTO files
                 (path, loc, modified, imports, dependencies, named_imports,
                  namespace_imports, function_names, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        } else {
            "INSERT OR REPLACE INTO files
                 (path, loc, modified, imports, dependencies, named_imports,
                  namespace_imports, function_names, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        };
        tx.prepare_cached(sql)?
            .execute(params![
                row.rel_path,
                row.loc,
                row.mtime,
                row.imports_json,
                row.deps_json,
                row.named_imports_json,
                row.namespace_imports_json,
                row.function_names_json,
                row.indexed_at,
            ])
            .context("Failed to upsert file row")?;
    }

    {
        let sql = if plain_insert {
            "INSERT INTO exports (name, file_path, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4)"
        } else {
            "INSERT OR REPLACE INTO exports (name, file_path, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4)"
        };
        let mut stmt = tx.prepare_cached(sql)?;
        for e in &row.exports {
            stmt.execute(params![e.name, row.rel_path, e.start_line, e.end_line])?;
        }
    }

    {
        let sql = if plain_insert {
            "INSERT INTO methods (dotted_name, file_path, start_line, end_line, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)"
        } else {
            "INSERT OR REPLACE INTO methods (dotted_name, file_path, start_line, end_line, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)"
        };
        let mut stmt = tx.prepare_cached(sql)?;
        for m in &row.methods {
            stmt.execute(params![
                m.dotted_name,
                row.rel_path,
                m.start_line,
                m.end_line,
                m.kind,
            ])?;
        }
    }

    Ok(())
}

/// Delete all rows from `files` (CASCADE clears `exports` and `methods`).
///
/// Used before a full-generate bulk INSERT to avoid per-row CASCADE overhead.
pub fn delete_all_files(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch("DELETE FROM files")
        .context("Failed to delete all files")
}

/// Insert or replace a complete file record plus its exports and methods.
///
/// Because the `files` table uses `INSERT OR REPLACE` with a PRIMARY KEY
/// on `path`, the old row is deleted first which cascades to `exports` and
/// `methods` — no manual cleanup needed.
pub fn upsert_file_data(
    tx: &Transaction<'_>,
    rel_path: &str,
    result: &ParseResult,
    mtime: Option<&str>,
) -> Result<()> {
    let meta = &result.metadata;
    let function_names = extract_function_names(result.custom_fields.as_ref());
    let indexed_at = Utc::now().to_rfc3339();

    tx.execute(
        "INSERT OR REPLACE INTO files
             (path, loc, modified, imports, dependencies, named_imports,
              namespace_imports, function_names, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            rel_path,
            meta.loc as i64,
            mtime,
            serde_json::to_string(&meta.imports).context("Failed to serialize imports")?,
            serde_json::to_string(&meta.dependencies)
                .context("Failed to serialize dependencies")?,
            serde_json::to_string(&meta.named_imports)
                .context("Failed to serialize named_imports")?,
            serde_json::to_string(&meta.namespace_imports)
                .context("Failed to serialize namespace_imports")?,
            serde_json::to_string(&function_names).context("Failed to serialize function_names")?,
            indexed_at,
        ],
    )
    .context("Failed to upsert file row")?;

    // Exports (top-level only — no parent_class)
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO exports (name, file_path, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for entry in &meta.exports {
            if entry.parent_class.is_none() {
                stmt.execute(params![
                    entry.name,
                    rel_path,
                    entry.start_line as i64,
                    entry.end_line as i64,
                ])?;
            }
        }
    }

    // Methods — deduplicate by dotted name (TypeScript overloads share the same
    // dotted name for each signature, deduplicated the same way as the YAML formatter).
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO methods (dotted_name, file_path, start_line, end_line, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut seen = std::collections::HashSet::new();
        for entry in &meta.exports {
            if let Some(ref class) = entry.parent_class {
                let key = format!("{}.{}", class, entry.name);
                if seen.insert(key.clone()) {
                    stmt.execute(params![
                        key,
                        rel_path,
                        entry.start_line as i64,
                        entry.end_line as i64,
                        entry.kind,
                    ])?;
                }
            }
        }
    }

    Ok(())
}

/// Load all file rows from the DB into a map keyed by relative path.
///
/// Only the fields needed for reverse-dependency computation are populated
/// (`imports`, `dependencies`, `named_imports`, `namespace_imports`, `function_names`).
/// The `exports` / `export_lines` / `methods` fields are left empty — they are
/// not needed by `build_reverse_deps`.
pub fn load_files_map(conn: &Connection) -> Result<HashMap<String, FileEntry>> {
    let mut stmt = conn.prepare(
        "SELECT path, loc, modified, imports, dependencies,
                named_imports, namespace_imports, function_names
         FROM files",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,         // path
            row.get::<_, i64>(1)?,            // loc
            row.get::<_, Option<String>>(2)?, // modified
            row.get::<_, Option<String>>(3)?, // imports
            row.get::<_, Option<String>>(4)?, // dependencies
            row.get::<_, Option<String>>(5)?, // named_imports
            row.get::<_, Option<String>>(6)?, // namespace_imports
            row.get::<_, Option<String>>(7)?, // function_names
        ))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (path, loc, modified, imports_j, deps_j, ni_j, ns_j, fn_j) = row?;

        let imports: Vec<String> = imports_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let dependencies: Vec<String> = deps_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let named_imports: HashMap<String, Vec<String>> = ni_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let namespace_imports: Vec<String> = ns_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let function_names: Vec<String> = fn_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        map.insert(
            path,
            FileEntry {
                exports: Vec::new(),
                export_lines: None,
                methods: None,
                imports,
                dependencies,
                loc: loc as usize,
                modified,
                function_names,
                named_imports,
                namespace_imports,
                ..Default::default()
            },
        );
    }

    Ok(map)
}

/// Load all file data from the DB, build a minimal `Manifest` (workspace
/// discovery included), recompute all reverse edges, and persist the results
/// to the `reverse_deps` table.
///
/// The DB stores file paths relative to `root`. `build_reverse_deps` requires
/// absolute paths for the cross-package resolver (oxc_resolver needs an
/// absolute importer path, Layer 3 checks paths against the filesystem, and
/// `canonicalize` only works on absolute paths). We therefore convert relative
/// keys → absolute before computation and strip `root` back to relative before
/// writing to the DB, so the stored paths stay consistent with the `files` table.
pub fn rebuild_and_write_reverse_deps(conn: &mut Connection, root: &Path) -> Result<()> {
    let rel_files_map = load_files_map(conn)?;

    let workspace_info = crate::resolver::workspace::discover(root);

    // Convert relative DB keys to absolute so the resolver works correctly.
    let abs_files_map: HashMap<String, crate::manifest::FileEntry> = rel_files_map
        .into_iter()
        .map(|(rel, entry)| {
            let abs = root.join(&rel).to_string_lossy().to_string();
            (abs, entry)
        })
        .collect();

    let mut manifest = crate::manifest::Manifest::new();
    manifest.files = abs_files_map;
    manifest.workspace_packages = workspace_info.packages;
    manifest.workspace_roots = workspace_info.roots;
    manifest.rebuild_reverse_deps();

    // Strip root prefix back to relative for consistent DB storage.
    let rel_reverse_deps: HashMap<String, Vec<String>> = manifest
        .reverse_deps
        .into_iter()
        .filter_map(|(abs_target, abs_sources)| {
            let rel_target = Path::new(&abs_target)
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .to_string();
            let rel_sources: Vec<String> = abs_sources
                .into_iter()
                .filter_map(|s| {
                    Path::new(&s)
                        .strip_prefix(root)
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                })
                .collect();
            Some((rel_target, rel_sources))
        })
        .collect();

    write_reverse_deps(conn, &rel_reverse_deps)
}

/// Clear the `reverse_deps` table and replace it with `rev_deps`.
pub fn write_reverse_deps(
    conn: &mut Connection,
    rev_deps: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM reverse_deps")?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO reverse_deps (target_path, source_path) VALUES (?1, ?2)",
        )?;
        for (target, sources) in rev_deps {
            for source in sources {
                stmt.execute(params![target, source])?;
            }
        }
    }
    tx.commit().context("Failed to commit reverse_deps")
}

/// Insert or replace workspace package entries.
pub fn upsert_workspace_packages(
    conn: &Connection,
    packages: &HashMap<String, std::path::PathBuf>,
) -> Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT OR REPLACE INTO workspace_packages (name, directory) VALUES (?1, ?2)",
    )?;
    for (name, dir) in packages {
        stmt.execute(params![name, dir.to_string_lossy().as_ref()])?;
    }
    Ok(())
}

/// Write a single key-value pair to the `meta` table.
pub fn write_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .context("Failed to write meta")?;
    Ok(())
}

// extract_function_names moved to fmm_core::types

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
