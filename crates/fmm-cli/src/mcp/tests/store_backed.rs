use super::super::McpServer;
use fmm_core::parser::{ExportEntry, Metadata, ParseResult};
use fmm_core::store::FmmStore;
use fmm_core::types::serialize_file_data;
use fmm_store::InMemoryStore;
use serde_json::json;
use std::path::PathBuf;

fn store_with_file(path: &str, metadata: Metadata) -> InMemoryStore {
    let store = InMemoryStore::new();
    let result = ParseResult {
        metadata,
        custom_fields: None,
    };
    let row = serialize_file_data(path, &result, None).unwrap();
    store.write_indexed_files(&[row], true).unwrap();
    store
}

#[test]
fn in_memory_store_mcp_list_exports() {
    let store = store_with_file(
        "src/app.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("createApp".into(), 1, 10),
                ExportEntry::new("AppConfig".into(), 12, 20),
            ],
            imports: vec!["react".into()],
            dependencies: vec!["./config".into()],
            loc: 25,
            ..Default::default()
        },
    );
    let server = McpServer::from_store(store, PathBuf::from("/test-project"));

    let response = server
        .call_tool("fmm_list_exports", json!({"file": "src/app.ts"}))
        .unwrap();
    let text = response["content"][0]["text"].as_str().unwrap();

    assert!(
        text.contains("createApp"),
        "InMemoryStore backed server must find createApp export; got:\n{text}",
    );
    assert!(
        text.contains("AppConfig"),
        "InMemoryStore backed server must find AppConfig export; got:\n{text}",
    );
}

#[test]
fn in_memory_store_mcp_lookup_export() {
    let store = store_with_file(
        "src/logger.ts",
        Metadata {
            exports: vec![ExportEntry::new("Logger".into(), 5, 30)],
            loc: 35,
            ..Default::default()
        },
    );
    let server = McpServer::from_store(store, PathBuf::from("/test-project"));

    let response = server
        .call_tool("fmm_lookup_export", json!({"name": "Logger"}))
        .unwrap();
    let text = response["content"][0]["text"].as_str().unwrap();

    assert!(
        text.contains("src/logger.ts"),
        "InMemoryStore backed lookup must resolve to src/logger.ts; got:\n{text}",
    );
}
