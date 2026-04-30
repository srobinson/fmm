use anyhow::{Context, Result};
use fmm_core::identity::{FileId, FileIdentityMap};
use fmm_core::manifest::Manifest;
use rusqlite::Connection;

pub(super) fn load_file_identity(conn: &Connection, manifest: &mut Manifest) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT file_id, path FROM file_paths ORDER BY file_id")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut entries = Vec::new();
    for row in rows {
        let (file_id, path) = row?;
        let file_id = u32::try_from(file_id).context("Stored file id is outside u32 range")?;
        entries.push((FileId(file_id), path));
    }

    if entries.is_empty() {
        return Ok(false);
    }

    manifest.set_file_identity(FileIdentityMap::from_file_id_paths(entries)?);
    Ok(true)
}
