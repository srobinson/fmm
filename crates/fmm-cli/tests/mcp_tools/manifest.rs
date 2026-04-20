use crate::support::setup_mcp_server;
use fmm_core::store::FmmStore;

#[test]
fn manifest_loads_from_db() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = fmm_store::SqliteStore::open(tmp.path())
        .unwrap()
        .load_manifest()
        .unwrap();
    assert_eq!(manifest.files.len(), 5);
}

#[test]
fn export_index_consistency() {
    let (tmp, _server) = setup_mcp_server();

    let manifest = fmm_store::SqliteStore::open(tmp.path())
        .unwrap()
        .load_manifest()
        .unwrap();
    for (export_name, file_path) in &manifest.export_index {
        let entry = manifest.files.get(file_path).unwrap_or_else(|| {
            panic!(
                "Export '{}' points to missing file '{}'",
                export_name, file_path
            )
        });
        assert!(
            entry.exports.contains(export_name),
            "File '{}' doesn't actually export '{}'",
            file_path,
            export_name
        );
    }
}
