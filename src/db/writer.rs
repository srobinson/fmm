use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use std::collections::HashMap;
use std::path::Path;

use crate::manifest::FileEntry;
use crate::parser::ParseResult;

/// Returns the file's last-modified time as an RFC3339 string, or `None`
/// if the metadata cannot be read.
pub fn file_mtime_rfc3339(path: &Path) -> Option<String> {
    use std::time::SystemTime;
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let secs = mtime.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64;
    let dt = chrono::DateTime::<Utc>::from_timestamp(secs, 0)?;
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
            "INSERT OR REPLACE INTO methods (dotted_name, file_path, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4)",
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
            },
        );
    }

    Ok(map)
}

/// Load all file data from the DB, build a minimal `Manifest` (workspace
/// discovery included), recompute all reverse edges, and persist the results
/// to the `reverse_deps` table.
pub fn rebuild_and_write_reverse_deps(conn: &mut Connection, root: &Path) -> Result<()> {
    let files_map = load_files_map(conn)?;

    let workspace_info = crate::resolver::workspace::discover(root);

    let mut manifest = crate::manifest::Manifest::new();
    manifest.files = files_map;
    manifest.workspace_packages = workspace_info.packages;
    manifest.workspace_roots = workspace_info.roots;
    manifest.rebuild_reverse_deps();

    write_reverse_deps(conn, &manifest.reverse_deps)
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

/// Extract `function_names` from a parser's `custom_fields` map.
///
/// TypeScript parsers emit `custom_fields["function_names"] = [...names]`.
/// Other languages may have different keys; returns an empty vec when absent.
fn extract_function_names(
    custom_fields: Option<&HashMap<String, serde_json::Value>>,
) -> Vec<String> {
    custom_fields
        .and_then(|cf| cf.get("function_names"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_or_create;
    use crate::parser::{ExportEntry, Metadata};
    use tempfile::TempDir;

    fn make_parse_result(
        exports: Vec<ExportEntry>,
        imports: Vec<String>,
        deps: Vec<String>,
    ) -> ParseResult {
        ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies: deps,
                loc: 10,
                ..Default::default()
            },
            custom_fields: None,
        }
    }

    #[test]
    fn upsert_and_query_file() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_parse_result(
            vec![ExportEntry::new("foo".into(), 1, 5)],
            vec!["react".into()],
            vec!["./utils".into()],
        );

        {
            let tx = conn.transaction().unwrap();
            upsert_file_data(
                &tx,
                "src/app.ts",
                &result,
                Some("2026-01-01T00:00:00+00:00"),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE path='src/app.ts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let loc: i64 = conn
            .query_row("SELECT loc FROM files WHERE path='src/app.ts'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(loc, 10);
    }

    #[test]
    fn exports_inserted_and_cascaded_on_replace() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_parse_result(
            vec![
                ExportEntry::new("Alpha".into(), 1, 10),
                ExportEntry::new("Beta".into(), 12, 20),
            ],
            vec![],
            vec![],
        );

        // First insert
        {
            let tx = conn.transaction().unwrap();
            upsert_file_data(&tx, "src/mod.ts", &result, None).unwrap();
            tx.commit().unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exports WHERE file_path='src/mod.ts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // Replace with single export — CASCADE should clear the old two
        let result2 =
            make_parse_result(vec![ExportEntry::new("Gamma".into(), 1, 5)], vec![], vec![]);
        {
            let tx = conn.transaction().unwrap();
            upsert_file_data(&tx, "src/mod.ts", &result2, None).unwrap();
            tx.commit().unwrap();
        }

        let count2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exports WHERE file_path='src/mod.ts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count2, 1);
    }

    #[test]
    fn methods_inserted_with_dedup() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_parse_result(
            vec![
                ExportEntry::method("run".into(), 5, 10, "Server".into()),
                // duplicate dotted name (overloads)
                ExportEntry::method("run".into(), 12, 14, "Server".into()),
            ],
            vec![],
            vec![],
        );

        {
            let tx = conn.transaction().unwrap();
            upsert_file_data(&tx, "src/server.ts", &result, None).unwrap();
            tx.commit().unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM methods WHERE file_path='src/server.ts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // Only first occurrence inserted (dedup)
        assert_eq!(count, 1);
    }

    #[test]
    fn is_file_up_to_date_returns_false_when_not_indexed() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();
        assert!(!is_file_up_to_date(
            &conn,
            "src/missing.ts",
            Some("2026-01-01T00:00:00+00:00")
        ));
    }

    #[test]
    fn is_file_up_to_date_returns_true_when_indexed_at_is_newer() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_parse_result(vec![], vec![], vec![]);
        {
            let tx = conn.transaction().unwrap();
            // indexed_at will be set to Utc::now()
            upsert_file_data(
                &tx,
                "src/file.ts",
                &result,
                Some("2020-01-01T00:00:00+00:00"),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        // Source mtime of 2020 — older than indexed_at (now) — so file is up to date
        assert!(is_file_up_to_date(
            &conn,
            "src/file.ts",
            Some("2020-01-01T00:00:00+00:00")
        ));
    }

    #[test]
    fn write_meta_roundtrip() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        write_meta(&conn, "fmm_version", "0.1.34").unwrap();

        let value: String = conn
            .query_row("SELECT value FROM meta WHERE key='fmm_version'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(value, "0.1.34");
    }

    #[test]
    fn upsert_workspace_packages_stored() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        let mut pkgs = HashMap::new();
        pkgs.insert(
            "shared".to_string(),
            std::path::PathBuf::from("/repo/packages/shared"),
        );

        upsert_workspace_packages(&conn, &pkgs).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workspace_packages WHERE name='shared'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn load_files_map_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_parse_result(vec![], vec!["react".into()], vec!["./helpers".into()]);
        {
            let tx = conn.transaction().unwrap();
            upsert_file_data(&tx, "src/component.tsx", &result, None).unwrap();
            tx.commit().unwrap();
        }

        let map = load_files_map(&conn).unwrap();
        let entry = map.get("src/component.tsx").unwrap();
        assert_eq!(entry.imports, vec!["react"]);
        assert_eq!(entry.dependencies, vec!["./helpers"]);
        assert_eq!(entry.loc, 10);
    }
}
