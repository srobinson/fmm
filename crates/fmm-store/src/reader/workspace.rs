use anyhow::Result;
use fmm_core::manifest::Manifest;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub(super) fn load_workspace_packages(
    conn: &Connection,
    root: &Path,
    manifest: &mut Manifest,
) -> Result<()> {
    let mut stmt = conn.prepare("SELECT name, directory FROM workspace_packages")?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (name, dir) = row?;
        let path = PathBuf::from(&dir);
        manifest.workspace_roots.push(path.clone());
        manifest.workspace_packages.insert(name, path);
    }

    let info = fmm_core::resolver::workspace::discover(root);
    if !info.packages.is_empty() || !info.roots.is_empty() {
        manifest.set_workspace_info(info);
    }

    Ok(())
}
