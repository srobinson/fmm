//! Read operations for loading a `Manifest` from the SQLite index.

mod exports;
mod files;
mod reverse_deps;
mod workspace;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use fmm_core::manifest::Manifest;

/// Build a complete `Manifest` by reading all tables from the open connection.
///
/// Applies the same TS > JS export collision logic so all consumers see
/// identical results regardless of which loader was used.
pub fn load_manifest_from_db(conn: &Connection, root: &Path) -> Result<Manifest> {
    let mut manifest = Manifest::new();

    files::load_files(conn, &mut manifest)?;
    exports::load_exports(conn, &mut manifest)?;
    exports::load_methods(conn, &mut manifest)?;
    reverse_deps::load_reverse_deps(conn, &mut manifest)?;
    workspace::load_workspace_packages(conn, root, &mut manifest)?;
    manifest.rebuild_file_identity()?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connection::open_or_create, writer};
    use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
    use std::collections::HashMap;
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
    fn loaded_manifest_assigns_sorted_file_ids() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();
        let result = make_result(vec![], vec![], vec![]);

        write_file(&mut conn, "src/z.ts", &result);
        write_file(&mut conn, "src/a.ts", &result);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

        assert_eq!(
            manifest.file_id("src/a.ts"),
            Some(fmm_core::identity::FileId(0))
        );
        assert_eq!(
            manifest.file_id("src/z.ts"),
            Some(fmm_core::identity::FileId(1))
        );
        assert_eq!(
            manifest.path_for_file_id(fmm_core::identity::FileId(0)),
            Some("src/a.ts")
        );
    }

    #[test]
    fn ts_wins_over_js_collision() {
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

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
        let dir = TempDir::new().unwrap();
        let mut conn = open_or_create(dir.path()).unwrap();

        let a = make_result(vec![ExportEntry::new("bar".into(), 1, 5)], vec![], vec![]);
        let b = make_result(vec![ExportEntry::new("bar".into(), 1, 5)], vec![], vec![]);

        write_file(&mut conn, "pkg/a.py", &a);
        write_file(&mut conn, "pkg/b.py", &b);

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();

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

        let a = make_result(vec![], vec![], vec!["./b".into()]);
        let b = make_result(vec![], vec![], vec![]);
        write_file(&mut conn, "src/b.ts", &b);
        write_file(&mut conn, "src/a.ts", &a);

        writer::rebuild_and_write_reverse_deps(&mut conn, dir.path()).unwrap();

        let manifest = load_manifest_from_db(&conn, dir.path()).unwrap();
        let deps = manifest.reverse_deps.get("src/b.ts").unwrap();
        assert!(deps.contains(&"src/a.ts".to_string()));
    }
}
