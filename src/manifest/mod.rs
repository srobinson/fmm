use anyhow::Result;
use chrono::{DateTime, Utc};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::parser::Metadata;

/// Typed representation of a `.fmm` sidecar file for serde_yaml deserialization.
/// Handles both v0.2 (exports as list) and v0.3 (exports as map with line ranges).
#[derive(Debug, Deserialize)]
struct SidecarData {
    file: String,
    #[serde(default)]
    exports: Option<serde_yaml::Value>,
    #[serde(default)]
    imports: Option<Vec<String>>,
    #[serde(default)]
    dependencies: Option<Vec<String>>,
    #[serde(default)]
    loc: Option<usize>,
    /// Captures all other fields (fmm version, modified, language-specific sections)
    #[serde(flatten)]
    _extra: HashMap<String, serde_yaml::Value>,
}

/// Line range for an export symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportLines {
    pub start: usize,
    pub end: usize,
}

/// Entry for a single file in the in-memory index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub exports: Vec<String>,
    /// Line ranges for exports (parallel to exports vec). None if from v0.2 sidecar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_lines: Option<Vec<ExportLines>>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}

impl From<Metadata> for FileEntry {
    fn from(metadata: Metadata) -> Self {
        let export_lines: Vec<ExportLines> = metadata
            .exports
            .iter()
            .map(|e| ExportLines {
                start: e.start_line,
                end: e.end_line,
            })
            .collect();
        let has_lines = export_lines.iter().any(|l| l.start > 0);
        Self {
            exports: metadata.exports.iter().map(|e| e.name.clone()).collect(),
            export_lines: if has_lines { Some(export_lines) } else { None },
            imports: metadata.imports,
            dependencies: metadata.dependencies,
            loc: metadata.loc,
        }
    }
}

/// Export index entry: file path + optional line range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLocation {
    pub file: String,
    pub lines: Option<ExportLines>,
}

/// In-memory index built from sidecar files.
/// No longer persisted to disk — built on-the-fly from `**/*.fmm` sidecars.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub generated: DateTime<Utc>,
    pub files: HashMap<String, FileEntry>,
    /// Maps export name -> file path (backward compat)
    pub export_index: HashMap<String, String>,
    /// Maps export name -> full location (file + lines)
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub export_locations: HashMap<String, ExportLocation>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            version: "2.0".to_string(),
            generated: Utc::now(),
            files: HashMap::new(),
            export_index: HashMap::new(),
            export_locations: HashMap::new(),
        }
    }

    /// Build an in-memory index by reading all `*.fmm` sidecar files under root.
    pub fn load_from_sidecars(root: &Path) -> Result<Self> {
        let mut manifest = Self::new();

        let walker = WalkBuilder::new(root).standard_filters(true).build();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("fmm") {
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some((file_path, file_entry)) = parse_sidecar(&content) {
                let key = if !file_path.is_empty() {
                    file_path
                } else {
                    let source_path = path.with_extension("");
                    source_path
                        .strip_prefix(root)
                        .unwrap_or(&source_path)
                        .display()
                        .to_string()
                };

                for (i, export) in file_entry.exports.iter().enumerate() {
                    manifest.export_index.insert(export.clone(), key.clone());
                    let lines = file_entry
                        .export_lines
                        .as_ref()
                        .and_then(|el| el.get(i))
                        .cloned();
                    manifest.export_locations.insert(
                        export.clone(),
                        ExportLocation {
                            file: key.clone(),
                            lines,
                        },
                    );
                }
                manifest.files.insert(key, file_entry);
            }
        }

        Ok(manifest)
    }

    /// Add or update a file entry in the index
    pub fn add_file(&mut self, path: &str, metadata: Metadata) {
        if let Some(old_entry) = self.files.get(path) {
            for old_export in &old_entry.exports {
                if self.export_index.get(old_export) == Some(&path.to_string()) {
                    self.export_index.remove(old_export);
                    self.export_locations.remove(old_export);
                }
            }
        }

        for export_entry in &metadata.exports {
            let should_insert = match self.export_index.get(&export_entry.name) {
                None => true,
                Some(existing) => {
                    let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                    let new_is_js = path.ends_with(".js") || path.ends_with(".jsx");
                    !(existing_is_ts && new_is_js)
                }
            };
            if should_insert {
                self.export_index
                    .insert(export_entry.name.clone(), path.to_string());
                let lines = if export_entry.start_line > 0 {
                    Some(ExportLines {
                        start: export_entry.start_line,
                        end: export_entry.end_line,
                    })
                } else {
                    None
                };
                self.export_locations.insert(
                    export_entry.name.clone(),
                    ExportLocation {
                        file: path.to_string(),
                        lines,
                    },
                );
            }
        }

        self.files
            .insert(path.to_string(), FileEntry::from(metadata));
    }

    pub fn remove_file(&mut self, path: &str) {
        if let Some(entry) = self.files.remove(path) {
            for export in entry.exports {
                self.export_index.remove(&export);
                self.export_locations.remove(&export);
            }
        }
    }

    pub fn touch(&mut self) {
        self.generated = Utc::now();
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn get_file(&self, path: &str) -> Option<&FileEntry> {
        self.files.get(path)
    }

    pub fn validate_file(&self, path: &str, current: &Metadata) -> bool {
        if let Some(entry) = self.files.get(path) {
            let current_names: Vec<String> =
                current.exports.iter().map(|e| e.name.clone()).collect();
            entry.exports == current_names
                && entry.imports == current.imports
                && entry.dependencies == current.dependencies
                && entry.loc == current.loc
        } else {
            false
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn file_paths(&self) -> Vec<&String> {
        self.files.keys().collect()
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a sidecar YAML file into (file_path, FileEntry).
/// Handles both v0.2 (exports as list) and v0.3 (exports as map with line ranges).
fn parse_sidecar(content: &str) -> Option<(String, FileEntry)> {
    let data: SidecarData = serde_yaml::from_str(content).ok()?;

    if data.file.is_empty() {
        return None;
    }

    let (exports, export_lines) = match data.exports {
        Some(serde_yaml::Value::Sequence(seq)) => {
            // v0.2 format: exports: [foo, bar]
            let names: Vec<String> = seq
                .into_iter()
                .filter_map(|v| match v {
                    serde_yaml::Value::String(s) => Some(s),
                    _ => None,
                })
                .collect();
            (names, None)
        }
        Some(serde_yaml::Value::Mapping(map)) => {
            // v0.3 format: exports:\n  foo: [1, 10]\n  bar: [12, 25]
            let mut names = Vec::new();
            let mut lines = Vec::new();
            for (key, value) in map {
                if let serde_yaml::Value::String(name) = key {
                    names.push(name);
                    match value {
                        serde_yaml::Value::Sequence(seq) if seq.len() == 2 => {
                            let start = seq[0].as_u64().unwrap_or(0) as usize;
                            let end = seq[1].as_u64().unwrap_or(0) as usize;
                            lines.push(ExportLines { start, end });
                        }
                        _ => {
                            lines.push(ExportLines { start: 0, end: 0 });
                        }
                    }
                }
            }
            let has_lines = lines.iter().any(|l| l.start > 0);
            (names, if has_lines { Some(lines) } else { None })
        }
        _ => (Vec::new(), None),
    };

    Some((
        data.file,
        FileEntry {
            exports,
            export_lines,
            imports: data.imports.unwrap_or_default(),
            dependencies: data.dependencies.unwrap_or_default(),
            loc: data.loc.unwrap_or(0),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ExportEntry;

    fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
        ExportEntry::new(name.to_string(), start, end)
    }

    #[test]
    fn test_manifest_add_file() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec![entry("validateUser", 5, 20), entry("createSession", 22, 45)],
            imports: vec!["crypto".to_string()],
            dependencies: vec!["./database".to_string()],
            loc: 234,
        };

        manifest.add_file("src/auth.ts", metadata);

        assert!(manifest.has_file("src/auth.ts"));
        assert_eq!(
            manifest.export_index.get("validateUser"),
            Some(&"src/auth.ts".to_string())
        );
        assert_eq!(
            manifest.export_index.get("createSession"),
            Some(&"src/auth.ts".to_string())
        );
        let loc = manifest.export_locations.get("validateUser").unwrap();
        assert_eq!(loc.lines.as_ref().unwrap().start, 5);
        assert_eq!(loc.lines.as_ref().unwrap().end, 20);
    }

    #[test]
    fn test_parse_sidecar_v03() {
        let content = "file: src/auth/session.ts\nfmm: v0.3\nexports:\n  createSession: [5, 20]\n  validateSession: [22, 45]\nimports: [jwt, redis]\ndependencies: [./types, ./config]\nloc: 234\nmodified: 2026-01-30";

        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/auth/session.ts");
        assert_eq!(entry.exports, vec!["createSession", "validateSession"]);
        let lines = entry.export_lines.unwrap();
        assert_eq!(lines[0], ExportLines { start: 5, end: 20 });
        assert_eq!(lines[1], ExportLines { start: 22, end: 45 });
    }

    #[test]
    fn test_parse_sidecar_v02_backward_compat() {
        let content = r#"file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession]
imports: [jwt, redis]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-30"#;

        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/auth/session.ts");
        assert_eq!(entry.exports, vec!["createSession", "validateSession"]);
        assert!(entry.export_lines.is_none());
        assert_eq!(entry.imports, vec!["jwt", "redis"]);
        assert_eq!(entry.dependencies, vec!["./types", "./config"]);
        assert_eq!(entry.loc, 234);
    }

    #[test]
    fn test_parse_sidecar_empty() {
        assert!(parse_sidecar("").is_none());
        assert!(parse_sidecar("loc: 10").is_none());
    }

    #[test]
    fn test_parse_sidecar_empty_exports() {
        let content = "file: src/empty.ts\nexports: []\nloc: 5\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/empty.ts");
        assert!(entry.exports.is_empty());
        assert_eq!(entry.loc, 5);
    }

    #[test]
    fn test_parse_sidecar_missing_optional_fields() {
        let content = "file: src/minimal.ts\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/minimal.ts");
        assert!(entry.exports.is_empty());
        assert!(entry.imports.is_empty());
        assert!(entry.dependencies.is_empty());
        assert_eq!(entry.loc, 0);
    }

    #[test]
    fn test_parse_sidecar_extra_fields() {
        let content = "file: src/lib.rs\nfmm: v0.3\nexports:\n  MyStruct: [5, 15]\nloc: 50\nrust:\n  derives: [Clone, Debug]\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/lib.rs");
        assert_eq!(entry.exports, vec!["MyStruct"]);
        assert_eq!(entry.loc, 50);
        let lines = entry.export_lines.unwrap();
        assert_eq!(lines[0], ExportLines { start: 5, end: 15 });
    }

    #[test]
    fn test_manifest_validate_file() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec![entry("test", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };

        manifest.add_file("file.ts", metadata.clone());
        assert!(manifest.validate_file("file.ts", &metadata));

        let different = Metadata {
            exports: vec![entry("different", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };
        assert!(!manifest.validate_file("file.ts", &different));
    }

    #[test]
    fn test_manifest_remove_file() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec![entry("toRemove", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        manifest.add_file("remove.ts", metadata);
        assert!(manifest.has_file("remove.ts"));
        assert!(manifest.export_index.contains_key("toRemove"));

        manifest.remove_file("remove.ts");
        assert!(!manifest.has_file("remove.ts"));
        assert!(!manifest.export_index.contains_key("toRemove"));
        assert!(!manifest.export_locations.contains_key("toRemove"));
    }

    #[test]
    fn test_manifest_update_file_cleans_old_exports() {
        let mut manifest = Manifest::new();

        let metadata1 = Metadata {
            exports: vec![entry("foo", 1, 5), entry("bar", 7, 10)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        manifest.add_file("file.ts", metadata1);

        let metadata2 = Metadata {
            exports: vec![entry("foo", 1, 5), entry("baz", 7, 12)],
            imports: vec![],
            dependencies: vec![],
            loc: 15,
        };

        manifest.add_file("file.ts", metadata2);

        assert_eq!(
            manifest.export_index.get("foo"),
            Some(&"file.ts".to_string())
        );
        assert_eq!(
            manifest.export_index.get("baz"),
            Some(&"file.ts".to_string())
        );
        assert!(!manifest.export_index.contains_key("bar"));
        assert_eq!(manifest.file_count(), 1);
    }
}
