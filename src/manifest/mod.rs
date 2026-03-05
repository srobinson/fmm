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
    /// Maps export name -> ALL definitions (no collision logic — every file that exports it).
    /// Used by the glossary to show all definitions and their dependents.
    #[serde(skip)]
    pub export_all: HashMap<String, Vec<ExportLocation>>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            version: "2.0".to_string(),
            generated: Utc::now(),
            files: HashMap::new(),
            export_index: HashMap::new(),
            export_locations: HashMap::new(),
            export_all: HashMap::new(),
        }
    }

    /// Build an in-memory index by reading all `*.fmm` sidecar files under root.
    pub fn load_from_sidecars(root: &Path) -> Result<Self> {
        let mut manifest = Self::new();

        // Sidecars are gitignored (users shouldn't commit them), but we still
        // need to find them. Overrides take precedence over .gitignore rules
        // while keeping node_modules/target/etc filtered out.
        let mut overrides = ignore::overrides::OverrideBuilder::new(root);
        overrides.add("*.fmm").expect("valid glob pattern");
        let walker = WalkBuilder::new(root)
            .standard_filters(true)
            .overrides(overrides.build().expect("valid overrides"))
            .build();

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
                    let lines = file_entry
                        .export_lines
                        .as_ref()
                        .and_then(|el| el.get(i))
                        .cloned();

                    // Unconditionally track all definitions for glossary
                    manifest
                        .export_all
                        .entry(export.clone())
                        .or_default()
                        .push(ExportLocation {
                            file: key.clone(),
                            lines: lines.clone(),
                        });

                    if let Some(existing) = manifest.export_index.get(export) {
                        if existing != &key {
                            let existing_is_ts =
                                existing.ends_with(".ts") || existing.ends_with(".tsx");
                            let new_is_ts = key.ends_with(".ts") || key.ends_with(".tsx");
                            let existing_is_js =
                                existing.ends_with(".js") || existing.ends_with(".jsx");
                            let new_is_js = key.ends_with(".js") || key.ends_with(".jsx");
                            if existing_is_ts && new_is_js {
                                // .js never overwrites .ts — expected, no warning
                                continue;
                            }
                            if !(existing_is_js && new_is_ts) {
                                eprintln!(
                                    "warning: export '{}' in {} shadows {}",
                                    export, key, existing
                                );
                            }
                        }
                    }
                    manifest.export_index.insert(export.clone(), key.clone());
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
                // Remove this file's entry from export_all
                let empty = if let Some(locations) = self.export_all.get_mut(old_export) {
                    locations.retain(|loc| loc.file != path);
                    locations.is_empty()
                } else {
                    false
                };
                if empty {
                    self.export_all.remove(old_export);
                }
            }
        }

        for export_entry in &metadata.exports {
            let lines = if export_entry.start_line > 0 {
                Some(ExportLines {
                    start: export_entry.start_line,
                    end: export_entry.end_line,
                })
            } else {
                None
            };

            // Unconditionally track all definitions for glossary
            self.export_all
                .entry(export_entry.name.clone())
                .or_default()
                .push(ExportLocation {
                    file: path.to_string(),
                    lines: lines.clone(),
                });

            let (should_insert, should_warn) = match self.export_index.get(&export_entry.name) {
                None => (true, false),
                Some(existing) if existing == path => (true, false),
                Some(existing) => {
                    let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                    let existing_is_js = existing.ends_with(".js") || existing.ends_with(".jsx");
                    let new_is_ts = path.ends_with(".ts") || path.ends_with(".tsx");
                    let new_is_js = path.ends_with(".js") || path.ends_with(".jsx");
                    if existing_is_ts && new_is_js {
                        // .js never overwrites .ts — expected, no warning
                        (false, false)
                    } else if existing_is_js && new_is_ts {
                        // .ts takes priority over .js — expected, no warning
                        (true, false)
                    } else {
                        (true, true)
                    }
                }
            };
            if should_warn {
                let old = &self.export_index[&export_entry.name];
                eprintln!(
                    "warning: export '{}' in {} shadows {}",
                    export_entry.name, path, old
                );
            }
            if should_insert {
                self.export_index
                    .insert(export_entry.name.clone(), path.to_string());
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
                let empty = if let Some(locations) = self.export_all.get_mut(&export) {
                    locations.retain(|loc| loc.file != path);
                    locations.is_empty()
                } else {
                    false
                };
                if empty {
                    self.export_all.remove(&export);
                }
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

/// Entry for a single export name in the glossary.
#[derive(Debug, Clone, Serialize)]
pub struct GlossaryEntry {
    pub name: String,
    pub sources: Vec<GlossarySource>,
}

/// One definition of a glossary export — the file it lives in, its line range,
/// and all files that import it.
#[derive(Debug, Clone, Serialize)]
pub struct GlossarySource {
    pub file: String,
    pub lines: Option<ExportLines>,
    pub used_by: Vec<String>,
}

/// Returns true if an export should be classified as a test artifact.
///
/// Checks symbol name conventions and file path conventions:
/// - Symbol starts with `test_` (Python) or `Test` (Go)
/// - File is under a test directory: `tests/`, `test/`, `__tests__/`
/// - File matches test file patterns: `_test.go`, `test_*.py`, `*_test.py`
fn is_test_export(name: &str, file: &str) -> bool {
    if name.starts_with("test_") || name.starts_with("Test") {
        return true;
    }
    let filename = file.rsplit('/').next().unwrap_or(file);
    if filename.ends_with("_test.go") {
        return true;
    }
    if filename.ends_with(".py")
        && (filename.starts_with("test_") || filename.ends_with("_test.py"))
    {
        return true;
    }
    file.starts_with("tests/")
        || file.starts_with("test/")
        || file.starts_with("__tests__/")
        || file.contains("/tests/")
        || file.contains("/test/")
        || file.contains("/__tests__/")
}

impl Manifest {
    /// Build the glossary: for each export name matching `pattern` (case-insensitive
    /// substring), collect all definitions and their dependents.
    /// Returns entries sorted alphabetically by name (case-insensitive).
    ///
    /// When `include_tests` is false (default), exports are filtered out where the
    /// symbol name follows test conventions (`test_`, `Test`) or the source file is
    /// under a test directory (`tests/`, `test/`, `__tests__/`, `_test.go`, etc.).
    pub fn build_glossary(&self, pattern: &str, include_tests: bool) -> Vec<GlossaryEntry> {
        let pat_lower = pattern.to_lowercase();
        let mut entries: Vec<GlossaryEntry> = self
            .export_all
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pat_lower))
            .filter_map(|(name, locations)| {
                let sources: Vec<GlossarySource> = locations
                    .iter()
                    .filter(|loc| include_tests || !is_test_export(name, &loc.file))
                    .map(|loc| {
                        let used_by = self.find_dependents(&loc.file);
                        GlossarySource {
                            file: loc.file.clone(),
                            lines: loc.lines.clone(),
                            used_by,
                        }
                    })
                    .collect();
                if sources.is_empty() {
                    None
                } else {
                    Some(GlossaryEntry {
                        name: name.clone(),
                        sources,
                    })
                }
            })
            .collect();
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        entries
    }

    /// Find all files that depend on `target_file` using all three dependency matchers:
    /// dep_matches (JS/TS/Rust/Go), python_dep_matches (Python relative), dotted_dep_matches (Python absolute).
    pub fn find_dependents(&self, target_file: &str) -> Vec<String> {
        let mut dependents: Vec<String> = self
            .files
            .iter()
            .filter(|(path, entry)| {
                let path = path.as_str();
                if path == target_file {
                    return false;
                }
                entry.dependencies.iter().any(|d| {
                    dep_matches(d, target_file, path) || python_dep_matches(d, target_file, path)
                }) || entry
                    .imports
                    .iter()
                    .any(|i| dotted_dep_matches(i, target_file))
            })
            .map(|(path, _)| path.clone())
            .collect();
        dependents.sort();
        dependents
    }
}

/// Check if a dependency path from `dependent_file` resolves to `target_file`.
/// Dependencies are stored as relative paths like "../utils/crypto.utils.js"
/// and need to be resolved against the dependent file's directory.
pub fn dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
    // Resolve the dependency path relative to the dependent file's directory
    let dep_dir = dependent_file
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");

    // Build resolved path by applying relative segments
    let mut parts: Vec<&str> = if dep_dir.is_empty() {
        Vec::new()
    } else {
        dep_dir.split('/').collect()
    };

    let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
    for segment in dep_clean.split('/') {
        if segment == ".." {
            parts.pop();
        } else if segment != "." {
            parts.push(segment);
        }
    }

    let resolved = parts.join("/");

    // Strip extension from both for comparison (.ts/.js/.tsx/.jsx interchangeable)
    let resolved_stem = resolved
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(&resolved);
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(target_file);

    if resolved_stem == target_stem {
        return true;
    }

    // Python packages: `./utils` should match `utils/__init__.py`
    if let Some(package_stem) = target_stem.strip_suffix("/__init__") {
        if resolved_stem == package_stem {
            return true;
        }
    }

    // Fallback: crate:: paths (Rust internal modules)
    // e.g. "crate::config" matches "src/config.rs"
    if let Some(module_path_str) = dep.strip_prefix("crate::") {
        let module_path = module_path_str.replace("::", "/");
        return target_stem.ends_with(&module_path);
    }

    // Fallback: domain-qualified paths (Go module paths, etc.)
    // e.g. "github.com/user/project/internal/handler" matches "internal/handler/handler.go"
    // Try progressively shorter path suffixes until one matches.
    if dep.contains('/') && !dep.starts_with('.') {
        let segments: Vec<&str> = dep.split('/').collect();
        for start in 1..segments.len() {
            let suffix = segments[start..].join("/");
            if target_stem.ends_with(&suffix) {
                return true;
            }
        }
    }

    false
}

fn resolve_python_relative_path(dep: &str, source_file: &str) -> Option<String> {
    debug_assert!(dep.starts_with('.') && !dep.starts_with("./"));
    let dots = dep.chars().take_while(|&c| c == '.').count();
    let module_name = &dep[dots..];

    let source_dir = source_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = if source_dir.is_empty() {
        vec![]
    } else {
        source_dir.split('/').collect()
    };

    // Single dot = current package; each additional dot = one level up
    for _ in 1..dots {
        parts.pop()?; // None if we'd go above the root
    }

    if module_name.is_empty() {
        // `from . import X` — no module name, can't pinpoint a file
        return None;
    }

    for part in module_name.split('.') {
        parts.push(part);
    }

    Some(parts.join("/"))
}

/// Match a Python-style relative import (`._run`, `..utils`) against a target
/// file path, given the dependent file's location. Used for downstream detection.
pub fn python_dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
    if !dep.starts_with('.') || dep.starts_with("./") || dep.starts_with("../") {
        return false;
    }
    if let Some(resolved) = resolve_python_relative_path(dep, dependent_file) {
        let target_stem = target_file
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(target_file);
        resolved == target_stem
    } else {
        false
    }
}

/// Match a Python absolute module import (`agno.models.message`) against a target
/// file path. Used for downstream detection.
///
/// Returns true when the dotted path resolves to the target file, considering
/// both root-relative paths (`agno/models/message.py`) and src-layout paths
/// (`src/agno/models/message.py`).
pub fn dotted_dep_matches(dep: &str, target_file: &str) -> bool {
    // Only handle dotted absolute imports — exclude relative (`.X`), paths (`/`), Rust (`::`)
    if dep.starts_with('.') || dep.contains('/') || dep.contains("::") || !dep.contains('.') {
        return false;
    }
    let path_stem = dep.replace('.', "/");
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(target_file);
    // Handle packages: `agno.models` resolves to `agno/models/__init__.py`
    let effective = target_stem.strip_suffix("/__init__").unwrap_or(target_stem);
    effective == path_stem.as_str() || effective.ends_with(&format!("/{}", path_stem))
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
    fn cross_file_collision_shadows_old_entry() {
        let mut manifest = Manifest::new();

        let meta_a = Metadata {
            exports: vec![entry("Config", 1, 10)],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        };
        let meta_b = Metadata {
            exports: vec![entry("Config", 5, 15)],
            imports: vec![],
            dependencies: vec![],
            loc: 30,
        };

        manifest.add_file("src/config/types.rs", meta_a);
        manifest.add_file("src/config/defaults.rs", meta_b);

        // Last writer wins
        assert_eq!(
            manifest.export_index.get("Config"),
            Some(&"src/config/defaults.rs".to_string())
        );
    }

    #[test]
    fn ts_over_js_priority_no_shadow() {
        let mut manifest = Manifest::new();

        let meta_ts = Metadata {
            exports: vec![entry("App", 1, 50)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };
        let meta_js = Metadata {
            exports: vec![entry("App", 1, 50)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };

        manifest.add_file("src/app.ts", meta_ts);
        manifest.add_file("src/app.js", meta_js);

        // .ts should win — .js doesn't overwrite
        assert_eq!(
            manifest.export_index.get("App"),
            Some(&"src/app.ts".to_string())
        );
    }

    #[test]
    fn js_then_ts_order_ts_still_wins() {
        let mut manifest = Manifest::new();

        let meta_js = Metadata {
            exports: vec![entry("App", 1, 50)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };
        let meta_ts = Metadata {
            exports: vec![entry("App", 1, 50)],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
        };

        // JS added first, then TS — TS should still win
        manifest.add_file("src/app.js", meta_js);
        manifest.add_file("src/app.ts", meta_ts);

        assert_eq!(
            manifest.export_index.get("App"),
            Some(&"src/app.ts".to_string())
        );
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

    // --- export_all / build_glossary / find_dependents tests ---

    #[test]
    fn export_all_tracks_all_definitions_including_duplicates() {
        let mut manifest = Manifest::new();

        let meta_a = Metadata {
            exports: vec![entry("Config", 1, 10)],
            imports: vec![],
            dependencies: vec![],
            loc: 20,
        };
        let meta_b = Metadata {
            exports: vec![entry("Config", 5, 15)],
            imports: vec![],
            dependencies: vec![],
            loc: 30,
        };

        manifest.add_file("src/config/types.rs", meta_a);
        manifest.add_file("src/config/defaults.rs", meta_b);

        // export_index last-writer-wins
        assert_eq!(
            manifest.export_index.get("Config"),
            Some(&"src/config/defaults.rs".to_string())
        );
        // export_all has both
        let all = manifest.export_all.get("Config").unwrap();
        assert_eq!(all.len(), 2);
        let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
        assert!(files.contains(&"src/config/types.rs"));
        assert!(files.contains(&"src/config/defaults.rs"));
    }

    #[test]
    fn build_glossary_returns_alphabetically_sorted_entries() {
        let mut manifest = Manifest::new();
        for (name, file) in [
            ("zebra", "z.ts"),
            ("alpha", "a.ts"),
            ("Config", "c.ts"),
            ("beta", "b.ts"),
        ] {
            manifest.add_file(
                file,
                Metadata {
                    exports: vec![entry(name, 1, 5)],
                    imports: vec![],
                    dependencies: vec![],
                    loc: 10,
                },
            );
        }

        let entries = manifest.build_glossary("a", true);
        // "alpha" matches; "zebra" matches (contains "a"); "beta" matches; "Config" does not
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        // Sorted case-insensitively
        assert!(names
            .windows(2)
            .all(|w| w[0].to_lowercase() <= w[1].to_lowercase()));
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"zebra"));
        assert!(names.contains(&"beta"));
        assert!(!names.contains(&"Config"));
    }

    #[test]
    fn build_glossary_case_insensitive_pattern() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/config.ts",
            Metadata {
                exports: vec![entry("AppConfig", 1, 5), entry("loadConfig", 7, 12)],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
            },
        );

        let entries = manifest.build_glossary("CONFIG", true);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"AppConfig"));
        assert!(names.contains(&"loadConfig"));
    }

    #[test]
    fn build_glossary_filters_test_exports_by_default() {
        let mut manifest = Manifest::new();
        // Normal export alongside a test function in the same file
        manifest.add_file(
            "src/agent.py",
            Metadata {
                exports: vec![entry("run_dispatch", 1, 50), entry("test_run_dispatch", 51, 80)],
                imports: vec![],
                dependencies: vec![],
                loc: 80,
            },
        );
        // Go test function (Test prefix) in a _test.go file
        manifest.add_file(
            "agent_test.go",
            Metadata {
                exports: vec![entry("TestRunDispatch", 1, 20)],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
            },
        );
        // Export under tests/ directory
        manifest.add_file(
            "tests/helpers.py",
            Metadata {
                exports: vec![entry("helper_fixture", 1, 10)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );

        // Default: include_tests=false — test artifacts excluded
        let entries = manifest.build_glossary("", false);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"run_dispatch"),
            "normal export should be included"
        );
        assert!(
            !names.contains(&"test_run_dispatch"),
            "test_ prefix should be excluded"
        );
        assert!(
            !names.contains(&"TestRunDispatch"),
            "Test prefix should be excluded"
        );
        assert!(
            !names.contains(&"helper_fixture"),
            "tests/ dir export should be excluded"
        );

        // With include_tests=true — all exports returned
        let entries_all = manifest.build_glossary("", true);
        let names_all: Vec<&str> = entries_all.iter().map(|e| e.name.as_str()).collect();
        assert!(names_all.contains(&"test_run_dispatch"));
        assert!(names_all.contains(&"TestRunDispatch"));
        assert!(names_all.contains(&"helper_fixture"));
    }

    #[test]
    fn is_test_export_covers_all_conventions() {
        // Symbol name prefix
        assert!(is_test_export("test_foo", "src/agent.py"));
        assert!(is_test_export("TestFoo", "agent.go"));
        assert!(!is_test_export("Config", "src/config.ts"));
        // Go test file
        assert!(is_test_export("anything", "agent_test.go"));
        assert!(!is_test_export("anything", "agent.go"));
        // Python test files
        assert!(is_test_export("foo", "test_agent.py"));
        assert!(is_test_export("foo", "agent_test.py"));
        assert!(!is_test_export("foo", "agent.py"));
        // Test directories
        assert!(is_test_export("foo", "tests/helpers.py"));
        assert!(is_test_export("foo", "test/fixtures.ts"));
        assert!(is_test_export("foo", "__tests__/utils.ts"));
        assert!(is_test_export("foo", "src/tests/helpers.py"));
        assert!(!is_test_export("foo", "src/config.ts"));
    }

    #[test]
    fn find_dependents_uses_dep_matches() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/config.ts",
            Metadata {
                exports: vec![entry("Config", 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );
        manifest.add_file(
            "src/app.ts",
            Metadata {
                exports: vec![entry("App", 1, 10)],
                imports: vec![],
                dependencies: vec!["./config".to_string()],
                loc: 20,
            },
        );
        manifest.add_file(
            "src/other.ts",
            Metadata {
                exports: vec![entry("Other", 1, 5)],
                imports: vec![],
                dependencies: vec!["./utils".to_string()],
                loc: 5,
            },
        );

        let deps = manifest.find_dependents("src/config.ts");
        assert_eq!(deps, vec!["src/app.ts"]);
    }

    #[test]
    fn export_all_remove_file_cleans_up() {
        let mut manifest = Manifest::new();

        manifest.add_file(
            "src/a.ts",
            Metadata {
                exports: vec![entry("Foo", 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );
        manifest.add_file(
            "src/b.ts",
            Metadata {
                exports: vec![entry("Foo", 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );

        assert_eq!(manifest.export_all.get("Foo").unwrap().len(), 2);

        manifest.remove_file("src/a.ts");
        let remaining = manifest.export_all.get("Foo").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].file, "src/b.ts");
    }

    #[test]
    fn export_all_remove_last_entry_cleans_key() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/only.ts",
            Metadata {
                exports: vec![entry("Unique", 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
            },
        );
        manifest.remove_file("src/only.ts");
        assert!(!manifest.export_all.contains_key("Unique"));
    }

    #[test]
    fn dep_matches_relative_path() {
        // dep "./types" from "src/index.ts" resolves to "src/types"
        assert!(dep_matches("./types", "src/types.ts", "src/index.ts"));
        assert!(dep_matches("./config", "src/config.ts", "src/index.ts"));
        assert!(!dep_matches("./types", "src/other.ts", "src/index.ts"));
    }

    #[test]
    fn dep_matches_nested_path() {
        // dep "./utils/helpers" from "src/index.ts" resolves to "src/utils/helpers"
        assert!(dep_matches(
            "./utils/helpers",
            "src/utils/helpers.ts",
            "src/index.ts"
        ));
        assert!(!dep_matches(
            "./utils/helpers",
            "src/utils/other.ts",
            "src/index.ts"
        ));
    }

    #[test]
    fn dep_matches_parent_relative() {
        // dep "../utils/crypto.utils.js" from "pkg/src/services/auth.service.ts"
        // resolves to "pkg/src/utils/crypto.utils"
        assert!(dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/services/auth.service.ts"
        ));
        assert!(!dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/services/other.ts",
            "pkg/src/services/auth.service.ts"
        ));
    }

    #[test]
    fn dep_matches_deep_parent_relative() {
        // dep "../../../utils/crypto.utils.js" from "pkg/src/tests/unit/auth/test.ts"
        // resolves to "pkg/src/utils/crypto.utils" (going up 3 dirs from tests/unit/auth)
        assert!(dep_matches(
            "../../../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/tests/unit/auth/test.ts"
        ));
    }

    #[test]
    fn dep_matches_without_prefix() {
        assert!(dep_matches("types", "src/types.ts", "src/index.ts"));
    }

    #[test]
    fn dep_matches_python_package() {
        // `./utils` should resolve to `utils/__init__.py` (Python package)
        assert!(dep_matches(
            "./utils",
            "src/utils/__init__.py",
            "src/service.py"
        ));
        // `../models` should resolve to `models/__init__.py` one level up
        assert!(dep_matches(
            "../models",
            "models/__init__.py",
            "src/service.py"
        ));
        // Should still match plain module file
        assert!(dep_matches("./utils", "src/utils.py", "src/service.py"));
        // No false positive: different package
        assert!(!dep_matches(
            "./utils",
            "src/auth/__init__.py",
            "src/service.py"
        ));
    }

    #[test]
    fn dep_matches_crate_path() {
        // Rust crate:: paths resolve via suffix matching
        assert!(dep_matches("crate::config", "src/config.rs", "src/main.rs"));
        assert!(dep_matches(
            "crate::parser::builtin",
            "src/parser/builtin.rs",
            "src/main.rs"
        ));
        // No false positives
        assert!(!dep_matches("crate::config", "src/other.rs", "src/main.rs"));
    }

    #[test]
    fn dep_matches_go_module_path() {
        // Go domain-qualified module paths resolve via suffix matching
        assert!(dep_matches(
            "github.com/user/project/internal/handler",
            "internal/handler/handler.go",
            "cmd/main.go"
        ));
        // Stdlib short paths don't match unrelated files
        assert!(!dep_matches(
            "fmt",
            "internal/format/format.go",
            "cmd/main.go"
        ));
    }
}
