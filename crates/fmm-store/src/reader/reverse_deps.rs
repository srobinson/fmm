use anyhow::Result;
use fmm_core::manifest::Manifest;
use rusqlite::Connection;

pub(super) fn load_reverse_deps(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare("SELECT target_path, source_path FROM reverse_deps")?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (target, source) = row?;
        manifest
            .reverse_deps
            .entry(target)
            .or_default()
            .push(source);
    }

    Ok(())
}
