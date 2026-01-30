use anyhow::Result;
use chrono::{DateTime, Utc};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::parser::Metadata;

/// Typed representation of a `.fmm` sidecar file for serde_yaml deserialization.
#[derive(Debug, Deserialize)]
struct SidecarData {
    file: String,
    #[serde(default)]
    exports: Option<Vec<String>>,
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

/// Entry for a single file in the in-memory index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}

impl From<Metadata> for FileEntry {
    fn from(metadata: Metadata) -> Self {
        Self {
            exports: metadata.exports,
            imports: metadata.imports,
            dependencies: metadata.dependencies,
            loc: metadata.loc,
        }
    }
}

/// In-memory index built from sidecar files.
/// No longer persisted to disk — built on-the-fly from `**/*.fmm` sidecars.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub generated: DateTime<Utc>,
    pub files: HashMap<String, FileEntry>,
    pub export_index: HashMap<String, String>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            version: "2.0".to_string(),
            generated: Utc::now(),
            files: HashMap::new(),
            export_index: HashMap::new(),
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

            if let Some((file_path, entry)) = parse_sidecar(&content) {
                // Use file_path from sidecar content, or derive from sidecar path
                let key = if !file_path.is_empty() {
                    file_path
                } else {
                    // Strip .fmm extension and make relative
                    let source_path = path.with_extension("");
                    source_path
                        .strip_prefix(root)
                        .unwrap_or(&source_path)
                        .display()
                        .to_string()
                };

                for export in &entry.exports {
                    manifest.export_index.insert(export.clone(), key.clone());
                }
                manifest.files.insert(key, entry);
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
                }
            }
        }

        for export in &metadata.exports {
            let should_insert = match self.export_index.get(export) {
                None => true,
                Some(existing) => {
                    let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                    let new_is_js = path.ends_with(".js") || path.ends_with(".jsx");
                    !(existing_is_ts && new_is_js)
                }
            };
            if should_insert {
                self.export_index.insert(export.clone(), path.to_string());
            }
        }

        self.files
            .insert(path.to_string(), FileEntry::from(metadata));
    }

    pub fn remove_file(&mut self, path: &str) {
        if let Some(entry) = self.files.remove(path) {
            for export in entry.exports {
                self.export_index.remove(&export);
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
            entry.exports == current.exports
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

/// Parse a sidecar YAML file into (file_path, FileEntry) using serde_yaml.
fn parse_sidecar(content: &str) -> Option<(String, FileEntry)> {
    let data: SidecarData = serde_yaml::from_str(content).ok()?;

    if data.file.is_empty() {
        return None;
    }

    Some((
        data.file,
        FileEntry {
            exports: data.exports.unwrap_or_default(),
            imports: data.imports.unwrap_or_default(),
            dependencies: data.dependencies.unwrap_or_default(),
            loc: data.loc.unwrap_or(0),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_add_file() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec!["validateUser".to_string(), "createSession".to_string()],
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
    }

    #[test]
    fn test_parse_sidecar() {
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
        assert_eq!(entry.imports, vec!["jwt", "redis"]);
        assert_eq!(entry.dependencies, vec!["./types", "./config"]);
        assert_eq!(entry.loc, 234);
    }

    #[test]
    fn test_parse_sidecar_empty() {
        assert!(parse_sidecar("").is_none());
        // Missing required `file` field
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
        let content = "file: src/lib.rs\nfmm: v0.2\nexports: [MyStruct]\nloc: 50\nrust:\n  derives: [Clone, Debug]\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/lib.rs");
        assert_eq!(entry.exports, vec!["MyStruct"]);
        assert_eq!(entry.loc, 50);
    }

    #[test]
    fn test_manifest_validate_file() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec!["test".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };

        manifest.add_file("file.ts", metadata.clone());
        assert!(manifest.validate_file("file.ts", &metadata));

        let different = Metadata {
            exports: vec!["different".to_string()],
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
            exports: vec!["toRemove".to_string()],
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
    }

    #[test]
    fn test_manifest_update_file_cleans_old_exports() {
        let mut manifest = Manifest::new();

        let metadata1 = Metadata {
            exports: vec!["foo".to_string(), "bar".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        manifest.add_file("file.ts", metadata1);

        let metadata2 = Metadata {
            exports: vec!["foo".to_string(), "baz".to_string()],
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
