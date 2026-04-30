//! `SqliteStore` implements `FmmStore` backed by a SQLite database.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::Connection;

use fmm_core::identity::Fingerprint;
use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_core::types::PreserializedRow;

use crate::error::StoreError;
use crate::{reader, writer};

/// SQLite-backed implementation of `FmmStore`.
///
/// Wraps a `Connection` with interior mutability (`RefCell`) so that all
/// `FmmStore` trait methods can take `&self` as required by the trait.
/// This is safe because `SqliteStore` is single-threaded (rusqlite
/// `Connection` is `!Send + !Sync`).
pub struct SqliteStore {
    conn: RefCell<Connection>,
    root: PathBuf,
}

impl SqliteStore {
    /// Open or create the fmm database at `root/.fmm.db`.
    ///
    /// Creates the schema if it does not exist, or migrates if the schema
    /// version has changed.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if the database cannot be opened or the schema
    /// cannot be applied.
    pub fn open_or_create(root: &Path) -> Result<Self, StoreError> {
        let conn = crate::connection::open_or_create(root)?;
        Ok(Self {
            conn: RefCell::new(conn),
            root: root.to_path_buf(),
        })
    }

    /// Open an existing fmm database at `root/.fmm.db`.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::NoIndex` if the database file does not exist.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        let conn = crate::connection::open_db(root)?;
        Ok(Self {
            conn: RefCell::new(conn),
            root: root.to_path_buf(),
        })
    }

    /// Open an existing database without version validation.
    ///
    /// Use for destructive operations (like `clean`) that must work
    /// regardless of which fmm version built the index.
    pub fn open_unchecked(root: &Path) -> Result<Self, StoreError> {
        let conn = crate::connection::open_db_unchecked(root)?;
        Ok(Self {
            conn: RefCell::new(conn),
            root: root.to_path_buf(),
        })
    }

    /// Returns the project root path this store was opened with.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the number of indexed files.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if the database cannot be queried.
    pub fn file_count(&self) -> Result<usize, StoreError> {
        let conn = self.conn.borrow();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Clear all indexed data (files, reverse deps, workspace packages).
    ///
    /// This is an administrative operation for `fmm clean`. Not part of the
    /// `FmmStore` trait because most store consumers should never call it.
    ///
    /// # Errors
    ///
    /// Returns `StoreError` if the database operation fails.
    pub fn clear_index(&self) -> Result<(), StoreError> {
        let conn = self.conn.borrow();
        conn.execute_batch(
            "DELETE FROM files; DELETE FROM reverse_deps; DELETE FROM workspace_packages;",
        )?;
        Ok(())
    }
}

impl FmmStore for SqliteStore {
    type Error = StoreError;

    fn load_manifest(&self) -> Result<Manifest, Self::Error> {
        let conn = self.conn.borrow();
        reader::load_manifest_from_db(&conn, &self.root).map_err(StoreError::Other)
    }

    fn load_fingerprints(&self) -> Result<HashMap<String, Fingerprint>, Self::Error> {
        let conn = self.conn.borrow();
        writer::load_fingerprints(&conn).map_err(StoreError::Other)
    }

    fn update_file_fingerprint(
        &self,
        rel_path: &str,
        fingerprint: &Fingerprint,
    ) -> Result<bool, Self::Error> {
        let conn = self.conn.borrow();
        writer::update_file_fingerprint(&conn, rel_path, fingerprint).map_err(StoreError::Other)
    }

    fn write_indexed_files(
        &self,
        rows: &[PreserializedRow],
        full_reindex: bool,
    ) -> Result<(), Self::Error> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;

        if full_reindex {
            writer::delete_all_files(&tx).map_err(StoreError::Other)?;
        }

        for row in rows {
            writer::upsert_preserialized(&tx, row, full_reindex).map_err(StoreError::Other)?;
        }

        tx.commit()?;
        Ok(())
    }

    fn upsert_single_file(&self, row: &PreserializedRow) -> Result<(), Self::Error> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        writer::upsert_preserialized(&tx, row, false).map_err(StoreError::Other)?;
        tx.commit()?;
        Ok(())
    }

    fn delete_single_file(&self, rel_path: &str) -> Result<bool, Self::Error> {
        let conn = self.conn.borrow();
        let rows = conn.execute(
            "DELETE FROM files WHERE path = ?1",
            rusqlite::params![rel_path],
        )?;
        Ok(rows > 0)
    }

    fn rebuild_and_write_reverse_deps(&self, root: &Path) -> Result<(), Self::Error> {
        let mut conn = self.conn.borrow_mut();
        writer::rebuild_and_write_reverse_deps(&mut conn, root).map_err(StoreError::Other)
    }

    fn upsert_workspace_packages(
        &self,
        packages: &HashMap<String, PathBuf>,
    ) -> Result<(), Self::Error> {
        let conn = self.conn.borrow();
        writer::upsert_workspace_packages(&conn, packages).map_err(StoreError::Other)
    }

    fn write_meta(&self) -> Result<(), Self::Error> {
        let conn = self.conn.borrow();
        writer::write_meta(&conn, "fmm_version", fmm_core::VERSION).map_err(StoreError::Other)?;
        writer::write_meta(&conn, "generated_at", &Utc::now().to_rfc3339())
            .map_err(StoreError::Other)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmm_core::identity::{Fingerprint, PARSER_CACHE_VERSION};
    use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
    use fmm_core::store::FmmStore;
    use fmm_core::types::serialize_file_data;
    use tempfile::TempDir;

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

    fn fingerprint() -> Fingerprint {
        Fingerprint {
            source_mtime: "2026-03-01T00:00:00+00:00".to_string(),
            source_size: 9,
            content_hash: "fnv1a64:test".to_string(),
            parser_cache_version: PARSER_CACHE_VERSION,
        }
    }

    #[test]
    fn sqlite_store_write_and_load_manifest() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

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
    }

    #[test]
    fn sqlite_store_batch_write_is_transactional() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

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
    fn sqlite_store_upsert_single_file() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        let result = make_parse_result(vec![ExportEntry::new("Foo".into(), 1, 5)]);
        let row = serialize_file_data("src/foo.ts", &result, None).unwrap();

        store.upsert_single_file(&row).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(manifest.files.contains_key("src/foo.ts"));
    }

    #[test]
    fn sqlite_store_delete_single_file() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        let result = make_parse_result(vec![ExportEntry::new("Bar".into(), 1, 5)]);
        let row = serialize_file_data("src/bar.ts", &result, None).unwrap();
        store.upsert_single_file(&row).unwrap();

        let deleted = store.delete_single_file("src/bar.ts").unwrap();
        assert!(deleted);

        let not_found = store.delete_single_file("src/bar.ts").unwrap();
        assert!(!not_found);
    }

    #[test]
    fn sqlite_store_load_fingerprints() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        let result = make_parse_result(vec![]);
        let mut row =
            serialize_file_data("src/x.ts", &result, Some("2026-03-01T00:00:00+00:00")).unwrap();
        row.fingerprint = Some(fingerprint());
        store.upsert_single_file(&row).unwrap();

        let fingerprints = store.load_fingerprints().unwrap();
        assert_eq!(fingerprints.get("src/x.ts"), Some(&fingerprint()));
    }

    #[test]
    fn sqlite_store_write_meta() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        store.write_meta().unwrap();

        let conn = store.conn.borrow();
        let version: String = conn
            .query_row("SELECT value FROM meta WHERE key='fmm_version'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, fmm_core::VERSION);

        let generated: String = conn
            .query_row("SELECT value FROM meta WHERE key='generated_at'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!generated.is_empty());
    }

    #[test]
    fn sqlite_store_upsert_workspace_packages() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        let mut pkgs = HashMap::new();
        pkgs.insert("core".to_string(), PathBuf::from("/repo/packages/core"));

        store.upsert_workspace_packages(&pkgs).unwrap();

        let conn = store.conn.borrow();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workspace_packages WHERE name='core'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn sqlite_store_full_reindex_clears_old_data() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::open_or_create(dir.path()).unwrap();

        // Write initial data
        let r1 = make_parse_result(vec![ExportEntry::new("Old".into(), 1, 5)]);
        let row1 = serialize_file_data("src/old.ts", &r1, None).unwrap();
        store.write_indexed_files(&[row1], true).unwrap();

        // Full reindex with different files
        let r2 = make_parse_result(vec![ExportEntry::new("New".into(), 1, 5)]);
        let row2 = serialize_file_data("src/new.ts", &r2, None).unwrap();
        store.write_indexed_files(&[row2], true).unwrap();

        let manifest = store.load_manifest().unwrap();
        assert!(!manifest.files.contains_key("src/old.ts"));
        assert!(manifest.files.contains_key("src/new.ts"));
    }
}
