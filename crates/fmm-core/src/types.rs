//! Domain types shared across fmm crates.
//!
//! These types are pure data structures with no database dependency.
//! They serve as the boundary types between fmm-core domain logic
//! and fmm-store persistence.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::{HashMap, HashSet};

use crate::parser::ParseResult;

/// Pre-serialized file data ready for batch insertion into a store.
///
/// Computing JSON strings is CPU-bound and can be done in parallel (rayon)
/// before the single-threaded SQLite transaction in Phase 3.
pub struct PreserializedRow {
    pub rel_path: String,
    pub loc: i64,
    pub mtime: Option<String>,
    pub imports_json: String,
    pub deps_json: String,
    pub named_imports_json: String,
    pub namespace_imports_json: String,
    pub function_names_json: String,
    pub indexed_at: String,
    pub exports: Vec<ExportRecord>,
    pub methods: Vec<MethodRecord>,
}

/// A flattened export entry ready for direct store insertion.
pub struct ExportRecord {
    pub name: String,
    pub start_line: i64,
    pub end_line: i64,
}

/// A flattened method entry ready for direct store insertion.
pub struct MethodRecord {
    pub dotted_name: String,
    pub start_line: i64,
    pub end_line: i64,
    /// ALP-922: NULL = class method, "nested-fn", "closure-state".
    pub kind: Option<String>,
}

/// Serialize all JSON fields for a parsed file. CPU-bound work safe to run in rayon.
///
/// Call this in parallel across dirty files, then pass the results to
/// the store's batch write method inside a transaction.
pub fn serialize_file_data(
    rel_path: &str,
    result: &ParseResult,
    mtime: Option<&str>,
) -> Result<PreserializedRow> {
    let meta = &result.metadata;
    let function_names = extract_function_names(result.custom_fields.as_ref());

    let exports: Vec<ExportRecord> = meta
        .exports
        .iter()
        .filter(|e| e.parent_class.is_none())
        .map(|e| ExportRecord {
            name: e.name.clone(),
            start_line: e.start_line as i64,
            end_line: e.end_line as i64,
        })
        .collect();

    let mut seen = HashSet::new();
    let methods: Vec<MethodRecord> = meta
        .exports
        .iter()
        .filter_map(|e| {
            e.parent_class.as_ref().and_then(|class| {
                let key = format!("{}.{}", class, e.name);
                if seen.insert(key.clone()) {
                    Some(MethodRecord {
                        dotted_name: key,
                        start_line: e.start_line as i64,
                        end_line: e.end_line as i64,
                        kind: e.kind.clone(),
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    Ok(PreserializedRow {
        rel_path: rel_path.to_string(),
        loc: meta.loc as i64,
        mtime: mtime.map(String::from),
        imports_json: serde_json::to_string(&meta.imports).context("serialize imports")?,
        deps_json: serde_json::to_string(&meta.dependencies).context("serialize dependencies")?,
        named_imports_json: serde_json::to_string(&meta.named_imports)
            .context("serialize named_imports")?,
        namespace_imports_json: serde_json::to_string(&meta.namespace_imports)
            .context("serialize namespace_imports")?,
        function_names_json: serde_json::to_string(&function_names)
            .context("serialize function_names")?,
        indexed_at: Utc::now().to_rfc3339(),
        exports,
        methods,
    })
}

pub fn extract_function_names(
    custom_fields: Option<&HashMap<String, serde_json::Value>>,
) -> Vec<String> {
    custom_fields
        .and_then(|cf| cf.get("function_names"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
