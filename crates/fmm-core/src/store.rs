//! Persistence port for fmm index storage.
//!
//! The `FmmStore` trait defines the boundary between domain logic (fmm-core)
//! and storage backends (fmm-store). All persistence operations go through
//! this trait, enabling SQLite, in-memory, or other backends.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::identity::Fingerprint;
use crate::manifest::Manifest;
use crate::types::PreserializedRow;

/// Persistence port for the fmm index.
///
/// Implementors provide storage for the parsed file index, export metadata,
/// reverse dependency graph, and workspace package mappings.
///
/// All methods take `&self` (not `&mut self`) because the primary backend
/// (SQLite in WAL mode) permits concurrent reads alongside a single writer.
/// Implementations that require interior mutability should use `RefCell`,
/// `Mutex`, or equivalent.
pub trait FmmStore {
    /// The error type returned by store operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Load a complete `Manifest` from the store.
    ///
    /// Populates all index fields: files, export_index, export_locations,
    /// export_all, method_index, reverse_deps, function_index, workspace_packages.
    ///
    /// # Errors
    ///
    /// Returns an error if the store is uninitialized or corrupt.
    fn load_manifest(&self) -> Result<Manifest, Self::Error>;

    /// Load complete stored fingerprints for each file path.
    ///
    /// Returns a map of `rel_path -> Fingerprint`. Used by the incremental
    /// indexer to detect which files need re-parsing or fingerprint refresh.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be read.
    fn load_fingerprints(&self) -> Result<HashMap<String, Fingerprint>, Self::Error>;

    /// Refresh stored fingerprint fields for a file without reparsing.
    ///
    /// Returns `true` when a row existed and was updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be updated.
    fn update_file_fingerprint(
        &self,
        rel_path: &str,
        fingerprint: &Fingerprint,
    ) -> Result<bool, Self::Error>;

    /// Write a batch of pre-serialized file rows to the store.
    ///
    /// When `full_reindex` is true, the implementation should clear all existing
    /// file data before inserting (using plain INSERT for performance). When false,
    /// each row should use upsert (INSERT OR REPLACE) semantics.
    ///
    /// The entire batch must be wrapped in a single transaction for atomicity.
    ///
    /// # Errors
    ///
    /// Returns an error if any row fails to write. Implementations should roll
    /// back the entire transaction on failure.
    fn write_indexed_files(
        &self,
        rows: &[PreserializedRow],
        full_reindex: bool,
    ) -> Result<(), Self::Error>;

    /// Upsert a single file's data (used by the file watcher for incremental updates).
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    fn upsert_single_file(&self, row: &PreserializedRow) -> Result<(), Self::Error>;

    /// Delete a single file's data from the store.
    ///
    /// Returns `true` if the file existed and was deleted, `false` if it was
    /// not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    fn delete_single_file(&self, rel_path: &str) -> Result<bool, Self::Error>;

    /// Rebuild reverse dependency mappings and write them to the store.
    ///
    /// Reads the current file state from the store, computes reverse deps
    /// using the cross-package resolver rooted at `root`, and persists the
    /// result.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or writing fails.
    fn rebuild_and_write_reverse_deps(&self, root: &Path) -> Result<(), Self::Error>;

    /// Upsert workspace package name-to-path mappings.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    fn upsert_workspace_packages(
        &self,
        packages: &HashMap<String, PathBuf>,
    ) -> Result<(), Self::Error>;

    /// Write store metadata (fmm version, timestamp).
    ///
    /// Reads the version from `fmm_core::VERSION` and computes the current
    /// timestamp. Implementations store these as key-value pairs.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata write fails.
    fn write_meta(&self) -> Result<(), Self::Error>;
}
