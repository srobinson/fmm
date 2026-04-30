//! In-memory `FmmStore` implementation for testing.
//!
//! `InMemoryStore` is a lightweight, `Send`-safe store that holds all index
//! data in memory. It replicates the SQLite round-trip behavior: JSON fields
//! are deserialized at write time and re-serialized on load, so callers see
//! identical `Manifest` structures regardless of backend.
//!
//! Gated behind the `test-support` feature. Not intended for production use.

mod manifest;
mod state;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_core::types::PreserializedRow;

use state::InnerState;

/// Error type for in-memory store operations.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MemoryStoreError(String);

impl MemoryStoreError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

/// In-memory `FmmStore` for testing MCP tool handlers without SQLite.
///
/// Uses `Mutex` for interior mutability, making this `Send + Sync`.
/// All trait methods acquire the lock briefly; no long-held borrows.
pub struct InMemoryStore {
    state: Mutex<InnerState>,
}

impl InMemoryStore {
    /// Create an empty in-memory store.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InnerState::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FmmStore for InMemoryStore {
    type Error = MemoryStoreError;

    fn load_manifest(&self) -> Result<Manifest, Self::Error> {
        let state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        if state.files.is_empty() {
            return Err(MemoryStoreError::new("empty store: no files indexed"));
        }

        Ok(manifest::build_manifest(&state))
    }

    fn load_indexed_mtimes(&self) -> Result<HashMap<String, String>, Self::Error> {
        let state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        let mtimes = state
            .files
            .iter()
            .filter_map(|(path, sf)| sf.mtime.as_ref().map(|m| (path.clone(), m.clone())))
            .collect();

        Ok(mtimes)
    }

    fn write_indexed_files(
        &self,
        rows: &[PreserializedRow],
        full_reindex: bool,
    ) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        if full_reindex {
            state.files.clear();
        }

        for row in rows {
            let (path, stored) = InnerState::ingest_row(row)?;
            state.files.insert(path, stored);
        }

        Ok(())
    }

    fn upsert_single_file(&self, row: &PreserializedRow) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        let (path, stored) = InnerState::ingest_row(row)?;
        state.files.insert(path, stored);
        Ok(())
    }

    fn delete_single_file(&self, rel_path: &str) -> Result<bool, Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        Ok(state.files.remove(rel_path).is_some())
    }

    fn rebuild_and_write_reverse_deps(&self, _root: &Path) -> Result<(), Self::Error> {
        Ok(())
    }

    fn upsert_workspace_packages(
        &self,
        packages: &HashMap<String, PathBuf>,
    ) -> Result<(), Self::Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| MemoryStoreError::new(format!("lock poisoned: {e}")))?;

        for (name, path) in packages {
            state.workspace_packages.insert(name.clone(), path.clone());
        }

        Ok(())
    }

    fn write_meta(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}
