use std::collections::HashMap;
use std::path::PathBuf;

use fmm_core::identity::Fingerprint;
use fmm_core::types::PreserializedRow;

use super::MemoryStoreError;

/// Stored representation of a single file's index data.
pub(super) struct StoredFile {
    pub(super) loc: i64,
    pub(super) mtime: Option<String>,
    pub(super) imports: Vec<String>,
    pub(super) dependencies: Vec<String>,
    pub(super) named_imports: HashMap<String, Vec<String>>,
    pub(super) namespace_imports: Vec<String>,
    pub(super) function_names: Vec<String>,
    pub(super) fingerprint: Option<Fingerprint>,
    pub(super) exports: Vec<StoredExport>,
    pub(super) methods: Vec<StoredMethod>,
}

pub(super) struct StoredExport {
    pub(super) name: String,
    pub(super) start_line: i64,
    pub(super) end_line: i64,
}

pub(super) struct StoredMethod {
    pub(super) dotted_name: String,
    pub(super) start_line: i64,
    pub(super) end_line: i64,
    pub(super) kind: Option<String>,
}

pub(super) struct InnerState {
    pub(super) files: HashMap<String, StoredFile>,
    pub(super) reverse_deps: HashMap<String, Vec<String>>,
    pub(super) workspace_packages: HashMap<String, PathBuf>,
}

impl InnerState {
    pub(super) fn new() -> Self {
        Self {
            files: HashMap::new(),
            reverse_deps: HashMap::new(),
            workspace_packages: HashMap::new(),
        }
    }

    /// Ingest a `PreserializedRow`, deserializing JSON fields into structured data.
    pub(super) fn ingest_row(
        row: &PreserializedRow,
    ) -> Result<(String, StoredFile), MemoryStoreError> {
        let imports: Vec<String> = serde_json::from_str(&row.imports_json)
            .map_err(|e| MemoryStoreError::new(format!("imports: {e}")))?;
        let dependencies: Vec<String> = serde_json::from_str(&row.deps_json)
            .map_err(|e| MemoryStoreError::new(format!("deps: {e}")))?;
        let named_imports: HashMap<String, Vec<String>> =
            serde_json::from_str(&row.named_imports_json)
                .map_err(|e| MemoryStoreError::new(format!("named_imports: {e}")))?;
        let namespace_imports: Vec<String> = serde_json::from_str(&row.namespace_imports_json)
            .map_err(|e| MemoryStoreError::new(format!("namespace_imports: {e}")))?;
        let function_names: Vec<String> = serde_json::from_str(&row.function_names_json)
            .map_err(|e| MemoryStoreError::new(format!("function_names: {e}")))?;

        let exports = row
            .exports
            .iter()
            .map(|e| StoredExport {
                name: e.name.clone(),
                start_line: e.start_line,
                end_line: e.end_line,
            })
            .collect();

        let methods = row
            .methods
            .iter()
            .map(|m| StoredMethod {
                dotted_name: m.dotted_name.clone(),
                start_line: m.start_line,
                end_line: m.end_line,
                kind: m.kind.clone(),
            })
            .collect();

        let stored = StoredFile {
            loc: row.loc,
            mtime: row.mtime.clone(),
            imports,
            dependencies,
            named_imports,
            namespace_imports,
            function_names,
            fingerprint: row.fingerprint.clone(),
            exports,
            methods,
        };

        Ok((row.rel_path.clone(), stored))
    }
}
