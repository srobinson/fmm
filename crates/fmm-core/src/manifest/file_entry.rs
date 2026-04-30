use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::parser::Metadata;

use super::ExportLines;

/// Entry for a single file in the in-memory index
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileEntry {
    pub exports: Vec<String>,
    /// Line ranges for exports (parallel to exports vec).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_lines: Option<Vec<ExportLines>>,
    /// Public class methods: `"ClassName.method"` → line range. Populated from the
    /// `methods:` sidecar section or from `ExportEntry` entries that have `parent_class` set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub methods: Option<HashMap<String, ExportLines>>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
    /// Last-modified date from the sidecar `modified:` field (YYYY-MM-DD). None if absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Names of exported module-level function declarations (TS/JS, Python, Rust).
    /// Populated from sidecar typescript.function_names section. Not persisted.
    /// Used to build function_index for call-site precision in fmm_glossary.
    #[serde(skip)]
    pub function_names: Vec<String>,
    /// Named imports per source module (TS/JS, Python, Rust). Key = import path as written in source.
    /// Value = original exported names (alias-resolved). Populated from sidecar named_imports section.
    /// Used by Layer 2 filtering in fmm_glossary.
    #[serde(skip)]
    pub named_imports: HashMap<String, Vec<String>>,
    /// Source paths of namespace imports and wildcard re-exports. Populated from sidecar.
    #[serde(skip)]
    pub namespace_imports: Vec<String>,
    /// ALP-922: depth-1 nested function declarations inside function bodies.
    /// dotted_name (e.g. "createTypeChecker.getIndexType") -> line range.
    /// Always shown in fmm_file_outline. Searchable via fmm_search.
    #[serde(skip)]
    pub nested_fns: HashMap<String, ExportLines>,
    /// ALP-922: depth-1 non-trivial prologue var/const/let declarations.
    /// dotted_name (e.g. "createTypeChecker.silentNeverType") -> line range.
    /// Shown only when include_private: true in fmm_file_outline.
    #[serde(skip)]
    pub closure_state: HashMap<String, ExportLines>,
}

impl From<Metadata> for FileEntry {
    fn from(metadata: Metadata) -> Self {
        let mut exports = Vec::new();
        let mut export_lines = Vec::new();
        let mut methods: HashMap<String, ExportLines> = HashMap::new();
        let mut nested_fns: HashMap<String, ExportLines> = HashMap::new();
        let mut closure_state: HashMap<String, ExportLines> = HashMap::new();

        for e in &metadata.exports {
            if let Some(ref parent) = e.parent_class {
                let key = format!("{}.{}", parent, e.name);
                let el = ExportLines {
                    start: e.start_line,
                    end: e.end_line,
                };
                match e.kind.as_deref() {
                    Some("nested-fn") => {
                        nested_fns.insert(key, el);
                    }
                    Some("closure-state") => {
                        closure_state.insert(key, el);
                    }
                    _ => {
                        methods.insert(key, el);
                    }
                }
            } else {
                exports.push(e.name.clone());
                export_lines.push(ExportLines {
                    start: e.start_line,
                    end: e.end_line,
                });
            }
        }

        let has_lines = export_lines.iter().any(|l| l.start > 0);
        Self {
            exports,
            export_lines: if has_lines { Some(export_lines) } else { None },
            methods: if methods.is_empty() {
                None
            } else {
                Some(methods)
            },
            imports: metadata.imports,
            dependencies: metadata.dependencies,
            loc: metadata.loc,
            modified: None,
            function_names: Vec::new(),
            named_imports: metadata.named_imports,
            namespace_imports: metadata.namespace_imports,
            nested_fns,
            closure_state,
        }
    }
}
