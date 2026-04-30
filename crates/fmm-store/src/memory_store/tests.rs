use super::*;
use fmm_core::identity::{Fingerprint, PARSER_CACHE_VERSION};
use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
use fmm_core::types::serialize_file_data;

fn make_parse_result(exports: Vec<ExportEntry>) -> ParseResult {
    ParseResult {
        metadata: Metadata {
            exports,
            imports: vec!["react".into()],
            dependencies: vec!["./utils".into()],
            loc: 15,
            ..Default::default()
        },
        custom_fields: None,
    }
}

fn fingerprint() -> Fingerprint {
    Fingerprint {
        source_mtime: "2026-03-01T00:00:00+00:00".to_string(),
        source_size: 9,
        content_hash: "fnv1a64:test".to_string(),
        parser_cache_version: PARSER_CACHE_VERSION,
    }
}

#[test]
fn write_and_load_manifest() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![
        ExportEntry::new("Alpha".into(), 1, 10),
        ExportEntry::new("Beta".into(), 12, 20),
    ]);
    let row =
        serialize_file_data("src/mod.ts", &result, Some("2026-01-01T00:00:00+00:00")).unwrap();

    store.write_indexed_files(&[row], true).unwrap();

    let manifest = store.load_manifest().unwrap();
    let entry = manifest.files.get("src/mod.ts").unwrap();
    assert_eq!(entry.loc, 15);
    assert!(entry.exports.contains(&"Alpha".to_string()));
    assert!(entry.exports.contains(&"Beta".to_string()));
    assert_eq!(
        manifest.export_index.get("Alpha").map(String::as_str),
        Some("src/mod.ts")
    );
}

#[test]
fn batch_write_is_atomic() {
    let store = InMemoryStore::new();

    let r1 = make_parse_result(vec![ExportEntry::new("A".into(), 1, 5)]);
    let r2 = make_parse_result(vec![ExportEntry::new("B".into(), 1, 5)]);
    let row1 = serialize_file_data("src/a.ts", &r1, None).unwrap();
    let row2 = serialize_file_data("src/b.ts", &r2, None).unwrap();

    store.write_indexed_files(&[row1, row2], true).unwrap();

    let manifest = store.load_manifest().unwrap();
    assert!(manifest.files.contains_key("src/a.ts"));
    assert!(manifest.files.contains_key("src/b.ts"));
}

#[test]
fn full_reindex_clears_old_data() {
    let store = InMemoryStore::new();

    let r1 = make_parse_result(vec![ExportEntry::new("Old".into(), 1, 5)]);
    let row1 = serialize_file_data("src/old.ts", &r1, None).unwrap();
    store.write_indexed_files(&[row1], true).unwrap();

    let r2 = make_parse_result(vec![ExportEntry::new("New".into(), 1, 5)]);
    let row2 = serialize_file_data("src/new.ts", &r2, None).unwrap();
    store.write_indexed_files(&[row2], true).unwrap();

    let manifest = store.load_manifest().unwrap();
    assert!(!manifest.files.contains_key("src/old.ts"));
    assert!(manifest.files.contains_key("src/new.ts"));
}

#[test]
fn upsert_single_file() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![ExportEntry::new("Foo".into(), 1, 5)]);
    let row = serialize_file_data("src/foo.ts", &result, None).unwrap();

    store.upsert_single_file(&row).unwrap();

    let manifest = store.load_manifest().unwrap();
    assert!(manifest.files.contains_key("src/foo.ts"));
}

#[test]
fn delete_single_file() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![ExportEntry::new("Bar".into(), 1, 5)]);
    let row = serialize_file_data("src/bar.ts", &result, None).unwrap();
    store.upsert_single_file(&row).unwrap();

    let deleted = store.delete_single_file("src/bar.ts").unwrap();
    assert!(deleted);

    let not_found = store.delete_single_file("src/bar.ts").unwrap();
    assert!(!not_found);
}

#[test]
fn load_fingerprints() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![]);
    let mut row =
        serialize_file_data("src/x.ts", &result, Some("2026-03-01T00:00:00+00:00")).unwrap();
    row.fingerprint = Some(fingerprint());
    store.upsert_single_file(&row).unwrap();

    let fingerprints = store.load_fingerprints().unwrap();
    assert_eq!(fingerprints.get("src/x.ts"), Some(&fingerprint()));
}

#[test]
fn empty_store_returns_error() {
    let store = InMemoryStore::new();
    assert!(store.load_manifest().is_err());
}

#[test]
fn is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<InMemoryStore>();
}

#[test]
fn ts_wins_over_js_collision() {
    let store = InMemoryStore::new();

    let js = make_parse_result(vec![ExportEntry::new("Widget".into(), 1, 5)]);
    let ts = make_parse_result(vec![ExportEntry::new("Widget".into(), 1, 5)]);
    let row_js = serialize_file_data("src/widget.js", &js, None).unwrap();
    let row_ts = serialize_file_data("src/widget.ts", &ts, None).unwrap();

    store.write_indexed_files(&[row_js, row_ts], true).unwrap();

    let manifest = store.load_manifest().unwrap();
    assert_eq!(
        manifest.export_index.get("Widget").map(String::as_str),
        Some("src/widget.ts")
    );
    assert_eq!(manifest.export_all.get("Widget").unwrap().len(), 2);
}

#[test]
fn methods_loaded_into_method_index() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![ExportEntry::method(
        "run".into(),
        5,
        15,
        "Server".into(),
    )]);
    let row = serialize_file_data("src/server.ts", &result, None).unwrap();

    store.write_indexed_files(&[row], true).unwrap();

    let manifest = store.load_manifest().unwrap();
    let loc = manifest.method_index.get("Server.run").unwrap();
    assert_eq!(loc.file, "src/server.ts");
    assert_eq!(loc.lines.as_ref().unwrap().start, 5);
}

#[test]
fn workspace_packages() {
    let store = InMemoryStore::new();

    let result = make_parse_result(vec![]);
    let row = serialize_file_data("src/lib.ts", &result, None).unwrap();
    store.upsert_single_file(&row).unwrap();

    let mut pkgs = HashMap::new();
    pkgs.insert("core".to_string(), PathBuf::from("/repo/packages/core"));
    store.upsert_workspace_packages(&pkgs).unwrap();

    let manifest = store.load_manifest().unwrap();
    assert!(manifest.workspace_packages.contains_key("core"));
}
