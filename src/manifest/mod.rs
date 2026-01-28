use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::parser::Metadata;

const MANIFEST_DIR: &str = ".fmm";
const MANIFEST_FILE: &str = "index.json";
const MANIFEST_VERSION: &str = "1.0";

/// Entry for a single file in the manifest
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

/// The complete manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub generated: DateTime<Utc>,
    pub files: HashMap<String, FileEntry>,
    pub export_index: HashMap<String, String>,
}

impl Manifest {
    /// Create a new empty manifest
    pub fn new() -> Self {
        Self {
            version: MANIFEST_VERSION.to_string(),
            generated: Utc::now(),
            files: HashMap::new(),
            export_index: HashMap::new(),
        }
    }

    /// Load manifest from disk, returns None if it doesn't exist
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let manifest_path = root.join(MANIFEST_DIR).join(MANIFEST_FILE);

        if !manifest_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&manifest_path).context("Failed to read manifest file")?;

        let manifest: Manifest =
            serde_json::from_str(&content).context("Failed to parse manifest JSON")?;

        Ok(Some(manifest))
    }

    /// Add or update a file entry in the manifest
    pub fn add_file(&mut self, path: &str, metadata: Metadata) {
        // Build export index entries
        for export in &metadata.exports {
            self.export_index.insert(export.clone(), path.to_string());
        }

        // Add the file entry
        self.files
            .insert(path.to_string(), FileEntry::from(metadata));
    }

    /// Remove a file from the manifest
    #[allow(dead_code)]
    pub fn remove_file(&mut self, path: &str) {
        if let Some(entry) = self.files.remove(path) {
            // Clean up export index
            for export in entry.exports {
                self.export_index.remove(&export);
            }
        }
    }

    /// Update the generated timestamp
    pub fn touch(&mut self) {
        self.generated = Utc::now();
    }

    /// Save manifest to disk
    pub fn save(&self, root: &Path) -> Result<()> {
        let manifest_dir = root.join(MANIFEST_DIR);

        // Create .fmm directory if it doesn't exist
        if !manifest_dir.exists() {
            fs::create_dir_all(&manifest_dir).context("Failed to create .fmm directory")?;
        }

        let manifest_path = manifest_dir.join(MANIFEST_FILE);
        let json = serde_json::to_string_pretty(self).context("Failed to serialize manifest")?;

        fs::write(&manifest_path, json).context("Failed to write manifest file")?;

        Ok(())
    }

    /// Check if a file exists in the manifest
    #[allow(dead_code)]
    pub fn has_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Get a file entry
    #[allow(dead_code)]
    pub fn get_file(&self, path: &str) -> Option<&FileEntry> {
        self.files.get(path)
    }

    /// Validate that manifest matches current file state
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

    /// Get the number of files in the manifest
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get all file paths in the manifest
    #[allow(dead_code)]
    pub fn file_paths(&self) -> Vec<&String> {
        self.files.keys().collect()
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    fn test_manifest_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec!["foo".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        manifest.add_file("test.ts", metadata);
        manifest.save(temp_dir.path()).unwrap();

        let loaded = Manifest::load(temp_dir.path()).unwrap().unwrap();
        assert!(loaded.has_file("test.ts"));
        assert_eq!(loaded.export_index.get("foo"), Some(&"test.ts".to_string()));
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
    fn test_manifest_export_index_duplicates() {
        // When same export name exists in multiple files, last one wins
        let mut manifest = Manifest::new();

        let metadata1 = Metadata {
            exports: vec!["sharedExport".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let metadata2 = Metadata {
            exports: vec!["sharedExport".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        };

        manifest.add_file("file1.ts", metadata1);
        manifest.add_file("file2.ts", metadata2);

        // Last file added wins for the export index
        assert_eq!(
            manifest.export_index.get("sharedExport"),
            Some(&"file2.ts".to_string())
        );
        // But both files exist in files map
        assert!(manifest.has_file("file1.ts"));
        assert!(manifest.has_file("file2.ts"));
    }

    #[test]
    fn test_manifest_json_serialization() {
        let mut manifest = Manifest::new();

        let metadata = Metadata {
            exports: vec!["myFunc".to_string()],
            imports: vec!["lodash".to_string()],
            dependencies: vec!["./utils".to_string()],
            loc: 100,
        };

        manifest.add_file("src/index.ts", metadata);

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&manifest).unwrap();

        // Verify camelCase serialization
        assert!(json.contains("\"exportIndex\""));
        assert!(json.contains("\"myFunc\""));
        assert!(json.contains("\"src/index.ts\""));

        // Deserialize back
        let loaded: Manifest = serde_json::from_str(&json).unwrap();
        assert!(loaded.has_file("src/index.ts"));
        assert_eq!(
            loaded.export_index.get("myFunc"),
            Some(&"src/index.ts".to_string())
        );
    }

    #[test]
    fn test_manifest_file_count() {
        let mut manifest = Manifest::new();
        assert_eq!(manifest.file_count(), 0);

        let metadata = Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        manifest.add_file("a.ts", metadata.clone());
        assert_eq!(manifest.file_count(), 1);

        manifest.add_file("b.ts", metadata.clone());
        assert_eq!(manifest.file_count(), 2);

        manifest.add_file("c.ts", metadata);
        assert_eq!(manifest.file_count(), 3);
    }
}
