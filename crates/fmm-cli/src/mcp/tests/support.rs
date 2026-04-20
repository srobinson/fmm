use super::super::McpServer;
use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_core::types::PreserializedRow;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) struct NullStore;

pub(super) type TestServer = McpServer<NullStore>;

impl FmmStore for NullStore {
    type Error = std::io::Error;

    fn load_manifest(&self) -> Result<Manifest, Self::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "NullStore: no manifest",
        ))
    }

    fn load_indexed_mtimes(&self) -> Result<HashMap<String, String>, Self::Error> {
        unimplemented!("NullStore")
    }

    fn write_indexed_files(
        &self,
        _rows: &[PreserializedRow],
        _full_reindex: bool,
    ) -> Result<(), Self::Error> {
        unimplemented!("NullStore")
    }

    fn upsert_single_file(&self, _row: &PreserializedRow) -> Result<(), Self::Error> {
        unimplemented!("NullStore")
    }

    fn delete_single_file(&self, _rel_path: &str) -> Result<bool, Self::Error> {
        unimplemented!("NullStore")
    }

    fn rebuild_and_write_reverse_deps(&self, _root: &Path) -> Result<(), Self::Error> {
        unimplemented!("NullStore")
    }

    fn upsert_workspace_packages(
        &self,
        _packages: &HashMap<String, PathBuf>,
    ) -> Result<(), Self::Error> {
        unimplemented!("NullStore")
    }

    fn write_meta(&self) -> Result<(), Self::Error> {
        unimplemented!("NullStore")
    }
}

pub(super) fn test_server(manifest: Manifest) -> TestServer {
    test_server_at(manifest, PathBuf::from("/tmp"))
}

pub(super) fn test_server_at(manifest: Manifest, root: PathBuf) -> TestServer {
    McpServer {
        store: None,
        manifest: Some(manifest),
        load_error: None,
        root,
    }
}

pub(super) fn tool_text(server: &McpServer<NullStore>, tool_name: &str, args: Value) -> String {
    let response = server.call_tool(tool_name, args).unwrap();
    response["content"][0]["text"].as_str().unwrap().to_string()
}

pub(super) fn assert_error(text: &str) {
    assert!(text.starts_with("ERROR:"), "expected ERROR:, got: {text}");
}

pub(super) fn list_files_order(server: &McpServer<NullStore>, args: Value) -> Vec<String> {
    tool_text(server, "fmm_list_files", args)
        .lines()
        .filter(|line| line.trim_start().starts_with("- "))
        .map(|line| {
            line.trim_start()
                .strip_prefix("- ")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}
