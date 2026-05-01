//! SQLite connection management for fmm databases.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

use crate::schema;

/// Database filename used by all fmm tooling.
pub const DB_FILENAME: &str = ".fmm.db";

/// Opens or creates the fmm SQLite database at `root/.fmm.db`.
///
/// On first call (no DB or missing schema), creates the database, applies
/// the schema, and writes the schema version to the `meta` table. On
/// subsequent calls, reads the stored schema version; if it mismatches,
/// drops all tables and recreates the schema (simple migration strategy).
pub fn open_or_create(root: &Path) -> Result<Connection> {
    let db_path = root.join(DB_FILENAME);
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;
    apply_pragmas(&conn)?;
    schema::ensure_schema(&conn)?;
    Ok(conn)
}

/// Opens an existing fmm database at `root/.fmm.db`.
///
/// Returns an error if the database file does not exist. Does not run
/// schema migrations.
pub fn open_db(root: &Path) -> Result<Connection> {
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!(
            "No fmm database found at {}. Run `fmm generate` first.",
            db_path.display()
        );
    }
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;
    apply_pragmas(&conn)?;
    check_schema_version_match(&conn)?;
    check_version_match(&conn)?;
    Ok(conn)
}

/// Opens an existing fmm database without version validation.
///
/// Use for operations (like `clean`) that must work regardless of
/// which fmm version built the index.
pub fn open_db_unchecked(root: &Path) -> Result<Connection> {
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!(
            "No fmm database found at {}. Run `fmm generate` first.",
            db_path.display()
        );
    }
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

fn check_version_match(conn: &Connection) -> Result<()> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='fmm_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    let running = fmm_core::VERSION;
    if let Some(ref v) = stored
        && v != running
    {
        anyhow::bail!(
            "Index was built with fmm v{} but you are running v{}. Run `fmm generate --force` to rebuild.",
            v,
            running
        );
    }
    Ok(())
}

fn check_schema_version_match(conn: &Connection) -> Result<()> {
    let stored = schema::read_schema_version(conn)?;
    match stored {
        Some(schema::SCHEMA_VERSION) => Ok(()),
        Some(version) => anyhow::bail!(
            "Index schema version {} does not match fmm schema version {}. Run `fmm generate --force` to rebuild.",
            version,
            schema::SCHEMA_VERSION
        ),
        None => anyhow::bail!(
            "Index schema version is missing or unreadable. Run `fmm generate --force` to rebuild."
        ),
    }
}

fn apply_pragmas(conn: &Connection) -> Result<()> {
    // busy_timeout: let concurrent CLI and MCP users drain short transactions.
    // journal_mode=WAL: allows concurrent readers while a writer is active.
    // synchronous=NORMAL: durable enough for a regeneratable index.
    // mmap_size=256MB: reduces syscall overhead on large repos.
    // temp_store=memory: scratch tables stay in RAM.
    // foreign_keys=ON: enforce ON DELETE CASCADE for exports/methods.
    // cache_size=-64000: 64MB page cache for bulk write performance.
    conn.execute_batch(
        "PRAGMA busy_timeout=5000;
         PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA mmap_size=268435456;
         PRAGMA temp_store=memory;
         PRAGMA foreign_keys=ON;
         PRAGMA cache_size=-64000;",
    )
    .context("Failed to apply database pragmas")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SCHEMA_VERSION;
    use tempfile::TempDir;

    fn create_old_files_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE files (
                 path TEXT PRIMARY KEY,
                 loc INTEGER NOT NULL,
                 modified TEXT,
                 imports TEXT,
                 dependencies TEXT,
                 named_imports TEXT,
                 namespace_imports TEXT,
                 function_names TEXT,
                 indexed_at TEXT NOT NULL
             );",
        )
        .unwrap();
    }

    #[test]
    fn open_or_create_creates_all_tables() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        for expected in &[
            "exports",
            "file_paths",
            "files",
            "meta",
            "methods",
            "reverse_deps",
            "workspace_packages",
        ] {
            assert!(
                tables.contains(&expected.to_string()),
                "missing table: {expected}"
            );
        }
    }

    #[test]
    fn schema_version_written_to_meta() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn open_or_create_is_idempotent() {
        let dir = TempDir::new().unwrap();
        drop(open_or_create(dir.path()).unwrap());
        let conn = open_or_create(dir.path()).unwrap();

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn wal_mode_is_active() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();

        assert_eq!(mode, "wal");
    }

    #[test]
    fn busy_timeout_is_configured() {
        let dir = TempDir::new().unwrap();
        let conn = open_or_create(dir.path()).unwrap();

        let timeout_ms: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();

        assert_eq!(timeout_ms, 5000);
    }

    #[test]
    fn schema_migration_on_version_mismatch() {
        let dir = TempDir::new().unwrap();

        // Manually create a DB with a fake old schema version.
        {
            let conn = Connection::open(dir.path().join(DB_FILENAME)).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '0');",
            )
            .unwrap();
        }

        // open_or_create should detect the mismatch and recreate the schema.
        let conn = open_or_create(dir.path()).unwrap();

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, SCHEMA_VERSION.to_string());

        // The files table should exist after recreation.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn open_db_errors_on_stale_schema_version() {
        let dir = TempDir::new().unwrap();
        let conn = Connection::open(dir.path().join(DB_FILENAME)).unwrap();
        create_old_files_table(&conn);
        conn.execute_batch(&format!(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta VALUES ('schema_version', '4');
             INSERT INTO meta VALUES ('fmm_version', '{}');",
            fmm_core::VERSION
        ))
        .unwrap();
        drop(conn);

        let error = open_db(dir.path()).unwrap_err().to_string();

        assert!(error.contains("schema version"));
        assert!(error.contains("Run `fmm generate --force`"));
        assert!(!error.contains("source_mtime"));
    }

    #[test]
    fn open_db_errors_when_meta_table_missing() {
        let dir = TempDir::new().unwrap();
        let conn = Connection::open(dir.path().join(DB_FILENAME)).unwrap();
        create_old_files_table(&conn);
        drop(conn);

        let error = open_db(dir.path()).unwrap_err().to_string();

        assert!(error.contains("missing or unreadable"));
        assert!(error.contains("Run `fmm generate --force`"));
    }

    #[test]
    fn open_or_create_rebuilds_existing_db_without_meta() {
        let dir = TempDir::new().unwrap();
        let conn = Connection::open(dir.path().join(DB_FILENAME)).unwrap();
        create_old_files_table(&conn);
        drop(conn);

        let conn = open_or_create(dir.path()).unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let source_mtime_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name='source_mtime'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, SCHEMA_VERSION.to_string());
        assert_eq!(source_mtime_columns, 1);
    }

    #[test]
    fn open_db_errors_when_no_db_file() {
        let dir = TempDir::new().unwrap();
        let result = open_db(dir.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Run `fmm generate`")
        );
    }
}
