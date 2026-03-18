use super::*;
use crate::connection::open_or_create;
use fmm_core::parser::{ExportEntry, Metadata};

use tempfile::TempDir;

fn make_parse_result(
    exports: Vec<ExportEntry>,
    imports: Vec<String>,
    deps: Vec<String>,
) -> ParseResult {
    ParseResult {
        metadata: Metadata {
            exports,
            imports,
            dependencies: deps,
            loc: 10,
            ..Default::default()
        },
        custom_fields: None,
    }
}

#[test]
fn upsert_and_query_file() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(
        vec![ExportEntry::new("foo".into(), 1, 5)],
        vec!["react".into()],
        vec!["./utils".into()],
    );

    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(
            &tx,
            "src/app.ts",
            &result,
            Some("2026-01-01T00:00:00+00:00"),
        )
        .unwrap();
        tx.commit().unwrap();
    }

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE path='src/app.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);

    let loc: i64 = conn
        .query_row("SELECT loc FROM files WHERE path='src/app.ts'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(loc, 10);
}

#[test]
fn exports_inserted_and_cascaded_on_replace() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(
        vec![
            ExportEntry::new("Alpha".into(), 1, 10),
            ExportEntry::new("Beta".into(), 12, 20),
        ],
        vec![],
        vec![],
    );

    // First insert
    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/mod.ts", &result, None).unwrap();
        tx.commit().unwrap();
    }

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM exports WHERE file_path='src/mod.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // Replace with single export
    let result2 = make_parse_result(vec![ExportEntry::new("Gamma".into(), 1, 5)], vec![], vec![]);
    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/mod.ts", &result2, None).unwrap();
        tx.commit().unwrap();
    }

    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM exports WHERE file_path='src/mod.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count2, 1);
}

#[test]
fn methods_inserted_with_dedup() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(
        vec![
            ExportEntry::method("run".into(), 5, 10, "Server".into()),
            // duplicate dotted name (overloads)
            ExportEntry::method("run".into(), 12, 14, "Server".into()),
        ],
        vec![],
        vec![],
    );

    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/server.ts", &result, None).unwrap();
        tx.commit().unwrap();
    }

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM methods WHERE file_path='src/server.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // Only first occurrence inserted (dedup)
    assert_eq!(count, 1);
}

#[test]
fn is_file_up_to_date_returns_false_when_not_indexed() {
    let dir = TempDir::new().unwrap();
    let conn = open_or_create(dir.path()).unwrap();
    assert!(!is_file_up_to_date(
        &conn,
        "src/missing.ts",
        Some("2026-01-01T00:00:00+00:00")
    ));
}

#[test]
fn is_file_up_to_date_returns_true_when_indexed_at_is_newer() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(vec![], vec![], vec![]);
    {
        let tx = conn.transaction().unwrap();
        // indexed_at will be set to Utc::now()
        upsert_file_data(
            &tx,
            "src/file.ts",
            &result,
            Some("2020-01-01T00:00:00+00:00"),
        )
        .unwrap();
        tx.commit().unwrap();
    }

    // Source mtime of 2020 is older than indexed_at (now)
    assert!(is_file_up_to_date(
        &conn,
        "src/file.ts",
        Some("2020-01-01T00:00:00+00:00")
    ));
}

#[test]
fn write_meta_roundtrip() {
    let dir = TempDir::new().unwrap();
    let conn = open_or_create(dir.path()).unwrap();

    write_meta(&conn, "fmm_version", "0.1.34").unwrap();

    let value: String = conn
        .query_row("SELECT value FROM meta WHERE key='fmm_version'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "0.1.34");
}

#[test]
fn upsert_workspace_packages_stored() {
    let dir = TempDir::new().unwrap();
    let conn = open_or_create(dir.path()).unwrap();

    let mut pkgs = HashMap::new();
    pkgs.insert(
        "shared".to_string(),
        std::path::PathBuf::from("/repo/packages/shared"),
    );

    upsert_workspace_packages(&conn, &pkgs).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM workspace_packages WHERE name='shared'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn load_files_map_roundtrip() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(vec![], vec!["react".into()], vec!["./helpers".into()]);
    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/component.tsx", &result, None).unwrap();
        tx.commit().unwrap();
    }

    let map = load_files_map(&conn).unwrap();
    let entry = map.get("src/component.tsx").unwrap();
    assert_eq!(entry.imports, vec!["react"]);
    assert_eq!(entry.dependencies, vec!["./helpers"]);
    assert_eq!(entry.loc, 10);
}

#[test]
fn delete_all_files_cascades_exports_and_methods() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(
        vec![
            ExportEntry::new("Foo".into(), 1, 5),
            ExportEntry::method("bar".into(), 6, 10, "Foo".into()),
        ],
        vec![],
        vec![],
    );

    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/a.ts", &result, None).unwrap();
        tx.commit().unwrap();
    }

    // Precondition: rows exist.
    let files: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(files, 1);
    let exports: i64 = conn
        .query_row("SELECT COUNT(*) FROM exports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(exports, 1);
    let methods: i64 = conn
        .query_row("SELECT COUNT(*) FROM methods", [], |r| r.get(0))
        .unwrap();
    assert_eq!(methods, 1);

    {
        let tx = conn.transaction().unwrap();
        delete_all_files(&tx).unwrap();
        tx.commit().unwrap();
    }

    // All three tables must be empty after CASCADE.
    let files: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(files, 0);
    let exports: i64 = conn
        .query_row("SELECT COUNT(*) FROM exports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(exports, 0);
    let methods: i64 = conn
        .query_row("SELECT COUNT(*) FROM methods", [], |r| r.get(0))
        .unwrap();
    assert_eq!(methods, 0);
}

#[test]
fn upsert_preserialized_plain_insert_roundtrip() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    let result = make_parse_result(
        vec![
            ExportEntry::new("Alpha".into(), 1, 10),
            ExportEntry::method("go".into(), 12, 20, "Alpha".into()),
        ],
        vec!["react".into()],
        vec!["./util".into()],
    );
    let row =
        serialize_file_data("src/comp.ts", &result, Some("2026-01-01T00:00:00+00:00")).unwrap();

    {
        let tx = conn.transaction().unwrap();
        upsert_preserialized(&tx, &row, true).unwrap();
        tx.commit().unwrap();
    }

    let loc: i64 = conn
        .query_row("SELECT loc FROM files WHERE path='src/comp.ts'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(loc, 10);

    let export_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM exports WHERE file_path='src/comp.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(export_count, 1);

    let method_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM methods WHERE file_path='src/comp.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(method_count, 1);
}

#[test]
fn full_generate_bulk_write_path() {
    let dir = TempDir::new().unwrap();
    let mut conn = open_or_create(dir.path()).unwrap();

    // Seed the DB with stale data for two files.
    let old = make_parse_result(vec![ExportEntry::new("Old".into(), 1, 5)], vec![], vec![]);
    {
        let tx = conn.transaction().unwrap();
        upsert_file_data(&tx, "src/a.ts", &old, None).unwrap();
        upsert_file_data(&tx, "src/b.ts", &old, None).unwrap();
        tx.commit().unwrap();
    }

    // Full generate: delete everything, then write only the new set.
    let new_result = make_parse_result(
        vec![ExportEntry::new("New".into(), 1, 8)],
        vec!["lodash".into()],
        vec![],
    );
    let row_a =
        serialize_file_data("src/a.ts", &new_result, Some("2026-06-01T00:00:00+00:00")).unwrap();

    {
        let tx = conn.transaction().unwrap();
        delete_all_files(&tx).unwrap();
        // Only src/a.ts is re-inserted
        upsert_preserialized(&tx, &row_a, true).unwrap();
        tx.commit().unwrap();
    }

    let export_name: String = conn
        .query_row(
            "SELECT name FROM exports WHERE file_path='src/a.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(export_name, "New");

    // src/b.ts must be gone
    let b_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE path='src/b.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(b_count, 0);
}
