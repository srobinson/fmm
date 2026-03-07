pub mod writer;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub const DB_FILENAME: &str = ".fmm.db";
const SCHEMA_VERSION: u32 = 1;

/// Opens or creates the fmm SQLite database at `root/.fmm.db`.
///
/// On first call (no DB or missing schema), creates the database, applies
/// the schema, and writes the schema version to the `meta` table. On
/// subsequent calls, reads the stored schema version; if it mismatches,
/// drops all tables and recreates the schema (simple migration strategy —
/// proper incremental migrations deferred to a future issue).
pub fn open_or_create(root: &Path) -> Result<Connection> {
    let db_path = root.join(DB_FILENAME);
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;
    apply_pragmas(&conn)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Opens an existing fmm database at `root/.fmm.db`.
///
/// Returns an error if the database file does not exist. Does not run
/// schema migrations — use `open_or_create` when generating.
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
    Ok(conn)
}

fn apply_pragmas(conn: &Connection) -> Result<()> {
    // journal_mode=WAL: allows concurrent readers while a writer is active.
    // synchronous=NORMAL: durable enough for a regeneratable index.
    // mmap_size=256MB: reduces syscall overhead on large repos.
    // temp_store=memory: scratch tables stay in RAM.
    // foreign_keys=ON: enforce ON DELETE CASCADE for exports/methods.
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA mmap_size=268435456;
         PRAGMA temp_store=memory;
         PRAGMA foreign_keys=ON;",
    )
    .context("Failed to apply database pragmas")?;
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    let version = read_schema_version(conn)?;
    if version == Some(SCHEMA_VERSION) {
        return Ok(());
    }
    if version.is_some() {
        // Version mismatch: nuke and rebuild. The database is a regeneratable
        // index, so data loss is acceptable — `fmm generate` will repopulate.
        drop_all_tables(conn)?;
    }
    create_schema(conn)?;
    write_schema_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

fn read_schema_version(conn: &Connection) -> Result<Option<u32>> {
    let meta_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .context("Failed to query sqlite_master")?;

    if !meta_exists {
        return Ok(None);
    }

    let version = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<u32>().ok());

    Ok(version)
}

fn write_schema_version(conn: &Connection, version: u32) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    )
    .context("Failed to write schema version")?;
    Ok(())
}

fn drop_all_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys=OFF;
         DROP TABLE IF EXISTS reverse_deps;
         DROP TABLE IF EXISTS methods;
         DROP TABLE IF EXISTS exports;
         DROP TABLE IF EXISTS files;
         DROP TABLE IF EXISTS workspace_packages;
         DROP TABLE IF EXISTS meta;
         PRAGMA foreign_keys=ON;",
    )
    .context("Failed to drop existing tables")?;
    Ok(())
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(CREATE_SCHEMA_SQL)
        .context("Failed to create database schema")?;
    Ok(())
}

const CREATE_SCHEMA_SQL: &str = "
-- Core file metadata. Replaces per-file .fmm YAML sidecars.
-- JSON columns are stored as JSON strings and deserialized on the Rust side.
CREATE TABLE IF NOT EXISTS files (
    path              TEXT PRIMARY KEY,
    loc               INTEGER NOT NULL,
    modified          TEXT,
    imports           TEXT,
    dependencies      TEXT,
    named_imports     TEXT,
    namespace_imports TEXT,
    function_names    TEXT,
    indexed_at        TEXT NOT NULL
);

-- Export locations. Replaces export_index, export_locations, export_all.
CREATE TABLE IF NOT EXISTS exports (
    name       TEXT NOT NULL,
    file_path  TEXT NOT NULL REFERENCES files(path) ON DELETE CASCADE,
    start_line INTEGER,
    end_line   INTEGER,
    PRIMARY KEY (name, file_path)
);
CREATE INDEX IF NOT EXISTS idx_exports_name ON exports(name);
CREATE INDEX IF NOT EXISTS idx_exports_file ON exports(file_path);

-- Class/interface methods for dotted-name lookups (e.g. 'MyClass.doThing').
-- Replaces the in-memory method_index.
CREATE TABLE IF NOT EXISTS methods (
    dotted_name TEXT NOT NULL,
    file_path   TEXT NOT NULL REFERENCES files(path) ON DELETE CASCADE,
    start_line  INTEGER,
    end_line    INTEGER,
    PRIMARY KEY (dotted_name, file_path)
);
CREATE INDEX IF NOT EXISTS idx_methods_name ON methods(dotted_name);

-- Pre-computed reverse dependency graph. Replaces O(N²) build_reverse_deps.
CREATE TABLE IF NOT EXISTS reverse_deps (
    target_path TEXT NOT NULL,
    source_path TEXT NOT NULL,
    PRIMARY KEY (target_path, source_path)
);
CREATE INDEX IF NOT EXISTS idx_reverse_deps_target ON reverse_deps(target_path);

-- Workspace package registry. Replaces the workspace_packages HashMap.
CREATE TABLE IF NOT EXISTS workspace_packages (
    name      TEXT PRIMARY KEY,
    directory TEXT NOT NULL
);

-- Manifest metadata: schema_version, fmm_version, generated_at.
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    fn open_db_errors_when_no_db_file() {
        let dir = TempDir::new().unwrap();
        let result = open_db(dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Run `fmm generate`"));
    }
}
