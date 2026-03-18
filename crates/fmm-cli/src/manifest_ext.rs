//! Extension functions for `Manifest` that depend on the database layer.
//!
//! These live in fmm-cli (not fmm-core) because they require rusqlite.
//! ALP-1485 will replace this with the `FmmStore` trait boundary.

use anyhow::Result;
use fmm_core::manifest::Manifest;
use std::path::Path;

use crate::db;

/// Load a complete `Manifest` from the SQLite database at `root/.fmm.db`.
///
/// Populates all index fields (export_index, export_locations, export_all,
/// method_index, reverse_deps, function_index, workspace_packages).
pub fn load_manifest_from_sqlite(root: &Path) -> Result<Manifest> {
    let conn = db::open_db(root)?;
    db::reader::load_manifest_from_db(&conn, root)
}

/// Load the `Manifest` from the SQLite index.
///
/// Returns an error if `.fmm.db` does not exist -- run `fmm generate` first.
/// All callers should use this function.
pub fn load_manifest(root: &Path) -> Result<Manifest> {
    load_manifest_from_sqlite(root)
}
