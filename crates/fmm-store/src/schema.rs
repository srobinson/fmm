//! Database schema definition and version management.

use anyhow::{Context, Result};
use rusqlite::Connection;

pub const SCHEMA_VERSION: u32 = 5;

pub fn ensure_schema(conn: &Connection) -> Result<()> {
    let version = read_schema_version(conn)?;
    if version == Some(SCHEMA_VERSION) {
        return Ok(());
    }
    if version.is_some() {
        // Version mismatch: nuke and rebuild. The database is a regeneratable
        // index, so data loss is acceptable.
        drop_all_tables(conn)?;
    }
    create_schema(conn)?;
    write_schema_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

pub fn read_schema_version(conn: &Connection) -> Result<Option<u32>> {
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

pub fn write_schema_version(conn: &Connection, version: u32) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    )
    .context("Failed to write schema version")?;
    Ok(())
}

pub fn drop_all_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys=OFF;
         DROP TABLE IF EXISTS reverse_deps;
         DROP TABLE IF EXISTS methods;
         DROP TABLE IF EXISTS exports;
         DROP TABLE IF EXISTS file_paths;
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

pub const CREATE_SCHEMA_SQL: &str = "
-- Core file metadata. Replaces per-file .fmm YAML sidecars.
-- JSON columns are stored as JSON strings and deserialized on the Rust side.
CREATE TABLE IF NOT EXISTS files (
    path              TEXT PRIMARY KEY,
    loc               INTEGER NOT NULL,
    modified          TEXT,
    imports           TEXT,
    dependencies      TEXT,
    dependency_kinds  TEXT,
    named_imports     TEXT,
    namespace_imports TEXT,
    function_names    TEXT,
    indexed_at        TEXT NOT NULL,
    source_mtime      TEXT,
    source_size       INTEGER,
    content_hash      TEXT,
    parser_cache_version INTEGER
);

-- Durable internal path identity. FileIds are rebuilt on full generate and
-- appended during watch updates so survivor ids stay stable within a session.
-- The UNIQUE constraint on `path` is what backs file_id_for_path lookups; no
-- explicit secondary index is needed because SQLite creates an implicit one
-- for UNIQUE.
CREATE TABLE IF NOT EXISTS file_paths (
    file_id INTEGER PRIMARY KEY,
    path    TEXT NOT NULL UNIQUE REFERENCES files(path) ON DELETE CASCADE
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

-- Class/interface methods and nested function symbols for dotted-name lookups.
-- kind: NULL = class method, 'nested-fn' = depth-1 nested function (ALP-922),
--        'closure-state' = depth-1 non-trivial prologue var (ALP-922).
CREATE TABLE IF NOT EXISTS methods (
    dotted_name TEXT NOT NULL,
    file_path   TEXT NOT NULL REFERENCES files(path) ON DELETE CASCADE,
    start_line  INTEGER,
    end_line    INTEGER,
    kind        TEXT,
    PRIMARY KEY (dotted_name, file_path)
);
CREATE INDEX IF NOT EXISTS idx_methods_name ON methods(dotted_name);

-- Pre-computed reverse dependency graph. Replaces O(N^2) build_reverse_deps.
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
