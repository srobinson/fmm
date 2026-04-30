use anyhow::Result;
use fmm_core::manifest::{FileEntry, Manifest};
use rusqlite::Connection;
use std::collections::HashMap;

pub(super) fn load_files(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT path, loc, modified, imports, dependencies,
                named_imports, namespace_imports, function_names
         FROM files",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    })?;

    for row in rows {
        let (path, loc, modified, imports_j, deps_j, ni_j, ns_j, fn_j) = row?;

        let imports: Vec<String> = imports_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let dependencies: Vec<String> = deps_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let named_imports: HashMap<String, Vec<String>> = ni_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let namespace_imports: Vec<String> = ns_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let function_names: Vec<String> = fn_j
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        manifest.files.insert(
            path,
            FileEntry {
                exports: Vec::new(),
                export_lines: None,
                methods: None,
                imports,
                dependencies,
                loc: loc as usize,
                modified,
                function_names,
                named_imports,
                namespace_imports,
                ..Default::default()
            },
        );
    }

    Ok(())
}
