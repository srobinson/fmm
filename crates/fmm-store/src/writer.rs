//! Write operations for the fmm SQLite index.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, Transaction, params};
use std::collections::HashMap;
use std::path::Path;

use fmm_core::identity::Fingerprint;
use fmm_core::manifest::FileEntry;
use fmm_core::parser::ParseResult;

// Re-export domain types from fmm-core for backward compatibility.
pub use fmm_core::types::{
    ExportRecord, MethodRecord, PreserializedRow, extract_function_names, serialize_file_data,
};

/// Load complete fingerprints from the DB in one query.
///
/// Used by the bulk staleness check in `fmm generate` to avoid individual
/// queries per file.
pub fn load_fingerprints(conn: &Connection) -> Result<HashMap<String, Fingerprint>> {
    let mut stmt = conn.prepare(
        "SELECT path, source_mtime, source_size, content_hash, parser_cache_version FROM files",
    )?;
    let mut map = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<u32>>(4)?,
        ))
    })?;

    for row in rows {
        let (path, source_mtime, source_size, content_hash, parser_cache_version) = row?;
        let Some(source_mtime) = source_mtime else {
            continue;
        };
        let Some(source_size) = source_size else {
            continue;
        };
        let Some(content_hash) = content_hash else {
            continue;
        };
        let Some(parser_cache_version) = parser_cache_version else {
            continue;
        };
        let Ok(source_size) = u64::try_from(source_size) else {
            continue;
        };
        map.insert(
            path,
            Fingerprint {
                source_mtime,
                source_size,
                content_hash,
                parser_cache_version,
            },
        );
    }

    Ok(map)
}

pub fn update_file_fingerprint(
    conn: &Connection,
    rel_path: &str,
    fingerprint: &Fingerprint,
) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE files
         SET modified = ?2,
             source_mtime = ?2,
             source_size = ?3,
             content_hash = ?4,
             parser_cache_version = ?5
         WHERE path = ?1",
        params![
            rel_path,
            fingerprint.source_mtime,
            i64::try_from(fingerprint.source_size).unwrap_or(i64::MAX),
            fingerprint.content_hash,
            fingerprint.parser_cache_version,
        ],
    )?;
    Ok(rows > 0)
}

/// Write a pre-serialized file row to the DB within an open transaction.
///
/// `plain_insert` controls whether to use `INSERT` (fast, caller must have
/// deleted existing rows first) or `INSERT OR REPLACE` (safe for incremental).
pub fn upsert_preserialized(
    tx: &Transaction<'_>,
    row: &PreserializedRow,
    plain_insert: bool,
) -> Result<()> {
    {
        let sql = if plain_insert {
            "INSERT INTO files
                 (path, loc, modified, imports, dependencies, named_imports,
                  namespace_imports, function_names, indexed_at, source_mtime,
                  source_size, content_hash, parser_cache_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        } else {
            "INSERT OR REPLACE INTO files
                 (path, loc, modified, imports, dependencies, named_imports,
                  namespace_imports, function_names, indexed_at, source_mtime,
                  source_size, content_hash, parser_cache_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        };
        let source_size = row
            .fingerprint
            .as_ref()
            .map(|fingerprint| i64::try_from(fingerprint.source_size).unwrap_or(i64::MAX));
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
                row.fingerprint
                    .as_ref()
                    .map(|fingerprint| &fingerprint.source_mtime),
                source_size,
                row.fingerprint
                    .as_ref()
                    .map(|fingerprint| &fingerprint.content_hash),
                row.fingerprint
                    .as_ref()
                    .map(|fingerprint| fingerprint.parser_cache_version),
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
/// Takes a `ParseResult` directly (not pre-serialized). Used by the file
/// watcher for single-file incremental updates.
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

    // Exports (top-level only)
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

    // Methods (deduplicated by dotted name)
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
/// Only the fields needed for reverse-dependency computation are populated.
pub fn load_files_map(conn: &Connection) -> Result<HashMap<String, FileEntry>> {
    let mut stmt = conn.prepare(
        "SELECT path, loc, modified, imports, dependencies,
                named_imports, namespace_imports, function_names
         FROM files",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
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

/// Load all file data from the DB, recompute reverse dependency edges,
/// and persist the results to the `reverse_deps` table.
///
/// Converts relative DB paths to absolute for the cross-package resolver,
/// then strips back to relative before writing.
pub fn rebuild_and_write_reverse_deps(conn: &mut Connection, root: &Path) -> Result<()> {
    let rel_files_map = load_files_map(conn)?;

    let workspace_info = fmm_core::resolver::workspace::discover(root);

    // Convert relative DB keys to absolute so the resolver works correctly.
    let abs_files_map: HashMap<String, FileEntry> = rel_files_map
        .into_iter()
        .map(|(rel, entry)| {
            let abs = root.join(&rel).to_string_lossy().to_string();
            (abs, entry)
        })
        .collect();

    let mut manifest = fmm_core::manifest::Manifest::new();
    manifest.files = abs_files_map;
    manifest.set_workspace_info(workspace_info);
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

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
