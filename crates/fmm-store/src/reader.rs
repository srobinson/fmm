//! Read operations for loading a `Manifest` from the SQLite index.

use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

use fmm_core::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};

/// Build a complete `Manifest` by reading all tables from the open connection.
///
/// Applies the same TS > JS export collision logic so all consumers see
/// identical results regardless of which loader was used.
pub fn load_manifest_from_db(conn: &Connection, root: &Path) -> Result<Manifest> {
    let mut manifest = Manifest::new();

    load_files(conn, &mut manifest)?;
    load_exports(conn, &mut manifest)?;
    load_methods(conn, &mut manifest)?;
    load_reverse_deps(conn, &mut manifest)?;
    load_workspace_packages(conn, root, &mut manifest)?;

    Ok(manifest)
}

fn load_files(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
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
                exports: Vec::new(), // populated by load_exports
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

fn load_exports(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT name, file_path, start_line, end_line
         FROM exports
         ORDER BY file_path, name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
        ))
    })?;

    // Collect by file so we can build FileEntry.exports + export_lines together
    let mut by_file: HashMap<String, Vec<(String, Option<ExportLines>)>> = HashMap::new();

    for row in rows {
        let (name, file_path, start, end) = row?;
        let lines = match (start, end) {
            (Some(s), Some(e)) if s > 0 => Some(ExportLines {
                start: s as usize,
                end: e as usize,
            }),
            _ => None,
        };
        by_file.entry(file_path).or_default().push((name, lines));
    }

    // Iterate in deterministic order so shadow warnings and index-building are
    // reproducible across runs. HashMap iteration is intentionally randomized.
    #[allow(clippy::type_complexity)]
    let mut sorted: Vec<(String, Vec<(String, Option<ExportLines>)>)> =
        by_file.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for (file_path, entries) in sorted {
        // Populate FileEntry.exports / export_lines
        let mut names: Vec<String> = Vec::with_capacity(entries.len());
        let mut line_ranges: Vec<ExportLines> = Vec::with_capacity(entries.len());
        let mut has_lines = false;

        for (name, lines) in &entries {
            names.push(name.clone());
            let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });
            if el.start > 0 {
                has_lines = true;
            }
            line_ranges.push(el);
        }

        if let Some(entry) = manifest.files.get_mut(&file_path) {
            entry.exports = names.clone();
            if has_lines {
                entry.export_lines = Some(line_ranges.clone());
            }
        }

        // Build the global indexes with TS > JS collision resolution
        for (i, (name, _lines)) in entries.iter().enumerate() {
            let line_range = if has_lines {
                line_ranges
                    .get(i)
                    .and_then(|l| if l.start > 0 { Some(l.clone()) } else { None })
            } else {
                None
            };

            // export_all: always track every definition
            manifest
                .export_all
                .entry(name.clone())
                .or_default()
                .push(ExportLocation {
                    file: file_path.clone(),
                    lines: line_range.clone(),
                });

            // function_index: first definition wins if this export is a known function
            if let Some(fe) = manifest.files.get(&file_path)
                && fe.function_names.contains(name)
            {
                manifest
                    .function_index
                    .entry(name.clone())
                    .or_insert(ExportLocation {
                        file: file_path.clone(),
                        lines: line_range.clone(),
                    });
            }

            // Re-exports (`from X import Y` + `__all__ = [Y]`) must not claim the
            // `export_index` slot or emit shadow warnings — the original definition
            // already owns that slot. Detection: this name appears as a value in
            // this file's `named_imports` (populated by `load_files` above).
            // `extract_named_imports` stores the original name for aliased imports
            // (`from X import A as B` → A), so aliased re-exports like
            // `manifest_write` naturally fall through and are treated as local
            // binds.
            let is_reexport = manifest
                .files
                .get(&file_path)
                .map(|fe| fe.named_imports.values().any(|v| v.contains(name)))
                .unwrap_or(false);
            if is_reexport {
                continue;
            }

            // Shadow is not a linter concern — the full list of definitions
            // for a name lives in `export_all`; consumers that care about
            // collisions query that. The only deterministic insert rule is
            // `.ts` > `.js`: .js must not overwrite .ts within the TS/JS
            // family. Everything else is last-one-wins.
            let should_insert = match manifest.export_index.get(name) {
                None => true,
                Some(existing) if existing == &file_path => true,
                Some(existing) => {
                    let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                    let new_is_js = file_path.ends_with(".js") || file_path.ends_with(".jsx");
                    !(existing_is_ts && new_is_js)
                }
            };

            if should_insert {
                manifest
                    .export_index
                    .insert(name.clone(), file_path.clone());
                manifest.export_locations.insert(
                    name.clone(),
                    ExportLocation {
                        file: file_path.clone(),
                        lines: line_range,
                    },
                );
            }
        }
    }

    Ok(())
}

fn load_methods(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt =
        conn.prepare("SELECT dotted_name, file_path, start_line, end_line, kind FROM methods")?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (dotted_name, file_path, start, end, kind) = row?;
        let lines = match (start, end) {
            (Some(s), Some(e)) if s > 0 => Some(ExportLines {
                start: s as usize,
                end: e as usize,
            }),
            _ => None,
        };

        let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });

        // Route into the correct FileEntry bucket based on kind.
        if let Some(fe) = manifest.files.get_mut(&file_path) {
            match kind.as_deref() {
                Some("nested-fn") => {
                    fe.nested_fns.insert(dotted_name.clone(), el);
                }
                Some("closure-state") => {
                    fe.closure_state.insert(dotted_name.clone(), el);
                }
                _ => {
                    // NULL kind = class method
                    fe.methods
                        .get_or_insert_with(HashMap::new)
                        .insert(dotted_name.clone(), el);
                }
            }
        }

        // All kinds go into method_index so fmm_read_symbol("Parent.child") works.
        manifest.method_index.insert(
            dotted_name,
            ExportLocation {
                file: file_path,
                lines,
            },
        );
    }

    Ok(())
}

fn load_reverse_deps(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
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

fn load_workspace_packages(conn: &Connection, root: &Path, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare("SELECT name, directory FROM workspace_packages")?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (name, dir) = row?;
        let path = std::path::PathBuf::from(&dir);
        manifest.workspace_roots.push(path.clone());
        manifest.workspace_packages.insert(name, path);
    }

    // If no workspace packages stored (e.g. not a monorepo), still discover
    // roots so downstream resolution works on first generate.
    if manifest.workspace_packages.is_empty() {
        let info = fmm_core::resolver::workspace::discover(root);
        manifest.workspace_packages = info.packages;
        manifest.workspace_roots = info.roots;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connection::open_or_create, writer};
    use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
    use tempfile::TempDir;

    fn make_result(
        exports: Vec<ExportEntry>,
        imports: Vec<String>,
        deps: Vec<String>,
    ) -> ParseResult {
        ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies: deps,
                loc: 20,
                ..Default::default()
            },
            custom_fields: None,
        }
    }

    fn write_file(conn: &mut rusqlite::Connection, rel_path: &str, result: &ParseResult) {
        let tx = conn.transaction().unwrap();
        writer::upsert_file_data(&tx, rel_path, result, None).unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn round_trip_files_and_exports() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_result(
            vec![
                ExportEntry::new("Alpha".into(), 1, 10),
                ExportEntry::new("Beta".into(), 12, 20),
            ],
            vec!["react".into()],
            vec!["./utils".into()],
        );
        write_file(&mut conn, "src/mod.ts", &result);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        let entry = manifest.files.get("src/mod.ts").unwrap();
        assert_eq!(entry.loc, 20);
        assert!(entry.exports.contains(&"Alpha".to_string()));
        assert!(entry.exports.contains(&"Beta".to_string()));

        assert_eq!(
            manifest.export_index.get("Alpha").map(String::as_str),
            Some("src/mod.ts")
        );
        assert_eq!(
            manifest
                .export_locations
                .get("Beta")
                .map(|l| l.file.as_str()),
            Some("src/mod.ts")
        );
        assert_eq!(manifest.export_all.get("Alpha").unwrap().len(), 1);
    }

    #[test]
    fn ts_wins_over_js_collision() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        // Insert JS file first, then TS
        let js = make_result(
            vec![ExportEntry::new("Widget".into(), 1, 5)],
            vec![],
            vec![],
        );
        let ts = make_result(
            vec![ExportEntry::new("Widget".into(), 1, 5)],
            vec![],
            vec![],
        );

        write_file(&mut conn, "src/widget.js", &js);
        write_file(&mut conn, "src/widget.ts", &ts);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        assert_eq!(
            manifest.export_index.get("Widget").map(String::as_str),
            Some("src/widget.ts")
        );
        // export_all has both
        assert_eq!(manifest.export_all.get("Widget").unwrap().len(), 2);
    }

    #[test]
    fn methods_loaded_into_method_index() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let result = make_result(
            vec![ExportEntry::method("run".into(), 5, 15, "Server".into())],
            vec![],
            vec![],
        );
        write_file(&mut conn, "src/server.ts", &result);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();
        let loc = manifest.method_index.get("Server.run").unwrap();
        assert_eq!(loc.file, "src/server.ts");
        assert_eq!(loc.lines.as_ref().unwrap().start, 5);
    }

    #[test]
    fn python_reexport_does_not_claim_export_index_slot_after_load() {
        // foo.py defines `bar` locally; __init__.py re-exports it. After DB
        // round-trip, export_index["bar"] must still point at foo.py.
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let foo = make_result(vec![ExportEntry::new("bar".into(), 1, 3)], vec![], vec![]);

        let mut named: HashMap<String, Vec<String>> = HashMap::new();
        named.insert(".foo".into(), vec!["bar".into()]);
        let init = ParseResult {
            metadata: Metadata {
                exports: vec![ExportEntry::new("bar".into(), 2, 2)],
                imports: vec![],
                dependencies: vec!["./foo".into()],
                loc: 3,
                named_imports: named,
                ..Default::default()
            },
            custom_fields: None,
        };

        write_file(&mut conn, "pkg/foo.py", &foo);
        write_file(&mut conn, "pkg/__init__.py", &init);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        assert_eq!(
            manifest.export_index.get("bar").map(String::as_str),
            Some("pkg/foo.py"),
            "re-export must not shadow original definition through DB load"
        );
        assert_eq!(manifest.export_all.get("bar").unwrap().len(), 2);
    }

    #[test]
    fn python_aliased_reexport_owns_its_own_slot_after_load() {
        // `from .foo import bar as baz` stores `bar` in named_imports, so the
        // local name `baz` is unique to __init__.py. __init__.py should own
        // export_index["baz"].
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let foo = make_result(vec![ExportEntry::new("bar".into(), 1, 3)], vec![], vec![]);

        let mut named: HashMap<String, Vec<String>> = HashMap::new();
        named.insert(".foo".into(), vec!["bar".into()]);
        let init = ParseResult {
            metadata: Metadata {
                exports: vec![ExportEntry::new("baz".into(), 2, 2)],
                imports: vec![],
                dependencies: vec!["./foo".into()],
                loc: 3,
                named_imports: named,
                ..Default::default()
            },
            custom_fields: None,
        };

        write_file(&mut conn, "pkg/foo.py", &foo);
        write_file(&mut conn, "pkg/__init__.py", &init);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        assert_eq!(
            manifest.export_index.get("baz").map(String::as_str),
            Some("pkg/__init__.py")
        );
        assert_eq!(
            manifest.export_index.get("bar").map(String::as_str),
            Some("pkg/foo.py")
        );
    }

    #[test]
    fn python_true_collision_still_shadows_after_load() {
        // Two files both define `bar` locally (no named_imports). Regression
        // guard: the re-export skip must not affect true collisions.
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let a = make_result(vec![ExportEntry::new("bar".into(), 1, 5)], vec![], vec![]);
        let b = make_result(vec![ExportEntry::new("bar".into(), 1, 5)], vec![], vec![]);

        write_file(&mut conn, "pkg/a.py", &a);
        write_file(&mut conn, "pkg/b.py", &b);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        // Last loaded (sorted) wins when both are real definitions. Sort
        // order is ascending file_path, so pkg/b.py is loaded after pkg/a.py
        // and wins the slot.
        assert_eq!(
            manifest.export_index.get("bar").map(String::as_str),
            Some("pkg/b.py")
        );
        assert_eq!(manifest.export_all.get("bar").unwrap().len(), 2);
    }

    #[test]
    fn reverse_deps_loaded() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        // a.ts depends on b.ts
        let a = make_result(vec![], vec![], vec!["./b".into()]);
        let b = make_result(vec![], vec![], vec![]);
        write_file(&mut conn, "src/b.ts", &b);
        write_file(&mut conn, "src/a.ts", &a);

        // Trigger reverse dep rebuild
        writer::rebuild_and_write_reverse_deps(&mut conn, dir.path()).unwrap();

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();
        let deps = manifest.reverse_deps.get("src/b.ts").unwrap();
        assert!(deps.contains(&"src/a.ts".to_string()));
    }
}
