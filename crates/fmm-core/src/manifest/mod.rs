use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::parser::Metadata;

pub mod call_site_finder;
pub mod private_members;

mod dependency_matcher;
mod glossary_builder;

use dependency_matcher::build_reverse_deps;

// Re-export public API consumed by other modules.
pub(crate) use dependency_matcher::{
    builtin_source_extensions, strip_source_ext, try_resolve_local_dep,
};
pub use dependency_matcher::{dep_matches, dotted_dep_matches, python_dep_matches};
pub use glossary_builder::{GlossaryEntry, GlossaryMode, GlossarySource};

/// Line range for an export symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportLines {
    pub start: usize,
    pub end: usize,
}

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

/// Export index entry: file path + optional line range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLocation {
    pub file: String,
    pub lines: Option<ExportLines>,
}

/// Classify a file path into a language family for cross-language collision
/// handling. Files in different families that define the same exported name
/// are treated as intentional parallel definitions (e.g. a Python dataclass
/// and a TypeScript interface mirroring the same API shape) and must not
/// trigger a shadow warning.
///
/// Families:
/// - `"python"` — `.py`
/// - `"js"` — `.js .jsx .mjs .cjs .ts .tsx`. TS > JS priority applies within
///   this family (see the collision-resolution call sites).
/// - `"rust"` — `.rs`
/// - `"go"` — `.go`
/// - otherwise — the extension itself (so distinct unknown extensions don't
///   lump together), or `""` if the path has no extension.
pub fn lang_family(path: &str) -> &str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match ext {
        "py" => "python",
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => "js",
        "rs" => "rust",
        "go" => "go",
        _ => ext,
    }
}

/// A re-export surfaced from another module, resolved to its origin definition.
///
/// Produced by [`Manifest::reexports_in_file`] and rendered by
/// `format_file_outline` into a separate `re-exports:` section so agents can
/// distinguish surface re-exports from local definitions at a glance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineReExport {
    pub name: String,
    pub origin_file: String,
    pub origin_start: usize,
    pub origin_end: usize,
}

/// In-memory index built from the SQLite database.
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
    /// Maps `"ClassName.method"` -> location for dotted symbol lookups.
    /// Built in-memory from `methods:` sidecar sections. Not persisted.
    #[serde(skip)]
    pub method_index: HashMap<String, ExportLocation>,
    /// Reverse dependency index: maps file path → all files that import it.
    /// Loaded from the pre-computed `reverse_deps` table for O(1) downstream lookups.
    /// Not persisted in this struct — loaded from DB on each `Manifest::load()` call.
    #[serde(skip)]
    pub reverse_deps: HashMap<String, Vec<String>>,
    /// Maps export name → file location for confirmed module-level function declarations (TS/JS).
    /// Enables O(1) "is this a bare function?" check at glossary query time.
    /// Only populated for exports listed in a file's `function_names` sidecar section.
    /// Not persisted — rebuilt on each load.
    #[serde(skip)]
    pub function_index: HashMap<String, ExportLocation>,
    /// Maps workspace package name → absolute directory path.
    /// Built from pnpm-workspace.yaml or package.json workspaces at load time.
    /// Not persisted — rebuilt on each load.
    #[serde(skip)]
    pub workspace_packages: HashMap<String, PathBuf>,
    /// Absolute directory paths of all workspace package roots.
    /// Used by the directory prefix heuristic (Layer 3 of cross-package resolution).
    /// Not persisted — rebuilt on each load.
    #[serde(skip)]
    pub workspace_roots: Vec<PathBuf>,
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
            method_index: HashMap::new(),
            reverse_deps: HashMap::new(),
            function_index: HashMap::new(),
            workspace_packages: HashMap::new(),
            workspace_roots: Vec::new(),
        }
    }

    // NOTE: `load()` and `load_from_sqlite()` live in fmm-cli as extension
    // methods on Manifest because they depend on the db module (rusqlite).
    // See ALP-1485 for the full decoupling via FmmStore trait.

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
            // Remove old method/nested-fn/closure-state entries for this file
            if let Some(ref old_methods) = old_entry.methods {
                for key in old_methods.keys() {
                    self.method_index.remove(key);
                }
            }
            for key in old_entry.nested_fns.keys() {
                self.method_index.remove(key);
            }
            for key in old_entry.closure_state.keys() {
                self.method_index.remove(key);
            }
        }

        for export_entry in &metadata.exports {
            // Method entries belong to a class — add to method_index, not export_index.
            if let Some(ref class) = export_entry.parent_class {
                let dotted = format!("{}.{}", class, export_entry.name);
                let lines = if export_entry.start_line > 0 {
                    Some(ExportLines {
                        start: export_entry.start_line,
                        end: export_entry.end_line,
                    })
                } else {
                    None
                };
                self.method_index.insert(
                    dotted,
                    ExportLocation {
                        file: path.to_string(),
                        lines,
                    },
                );
                continue;
            }

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

            // Re-exports (`from X import Y` + `__all__ = [Y]`) must not claim the
            // `export_index` slot or emit shadow warnings — the original definition
            // already owns that slot. Detection: this name appears as a value in
            // the file's `named_imports`. `extract_named_imports` stores the
            // original name for aliased imports (`from X import A as B` → A), so
            // aliased re-exports like `manifest_write` naturally fall through here
            // and are treated as local binds.
            if metadata
                .named_imports
                .values()
                .any(|names| names.contains(&export_entry.name))
            {
                continue;
            }

            let (should_insert, should_warn) = match self.export_index.get(&export_entry.name) {
                None => (true, false),
                Some(existing) if existing == path => (true, false),
                Some(existing) => {
                    let existing_family = lang_family(existing);
                    let new_family = lang_family(path);
                    if existing_family != new_family {
                        // Cross-language collision (e.g. Python dataclass mirrored
                        // as a TS interface for an API contract). Last one wins
                        // deterministically per insert order, but no warning —
                        // these are intentional API-surface mirrors, not shadows.
                        (true, false)
                    } else if existing_family == "js" {
                        // Within the JS family, TS takes priority over JS.
                        let existing_is_ts =
                            existing.ends_with(".ts") || existing.ends_with(".tsx");
                        let existing_is_js =
                            existing.ends_with(".js") || existing.ends_with(".jsx");
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
                    } else {
                        // Same non-JS family — real shadow collision.
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
            for export in &entry.exports {
                self.export_index.remove(export);
                self.export_locations.remove(export);
                let empty = if let Some(locations) = self.export_all.get_mut(export) {
                    locations.retain(|loc| loc.file != path);
                    locations.is_empty()
                } else {
                    false
                };
                if empty {
                    self.export_all.remove(export);
                }
            }
            if let Some(methods) = entry.methods {
                for key in methods.keys() {
                    self.method_index.remove(key);
                }
            }
            for key in entry.nested_fns.keys() {
                self.method_index.remove(key);
            }
            for key in entry.closure_state.keys() {
                self.method_index.remove(key);
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
            let current_names: Vec<String> = current
                .exports
                .iter()
                .filter(|e| e.parent_class.is_none())
                .map(|e| e.name.clone())
                .collect();
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

    /// Rebuild the reverse dependency index from the current file set.
    ///
    /// Called automatically by `load_from_sidecars`. Call this manually when
    /// building a manifest incrementally via `add_file` (e.g. in tests or
    /// benchmarks) to ensure downstream lookups are accurate.
    pub fn rebuild_reverse_deps(&mut self) {
        self.reverse_deps = build_reverse_deps(self);
    }

    /// Return the re-exports surfaced by `file`, each resolved to its origin
    /// definition. A re-export is an exported name whose string also appears
    /// as a value in the file's `named_imports` map (i.e. imported by name
    /// from another module and re-surfaced in this file's public API).
    ///
    /// Aliased imports like `from X import A as B` are NOT re-exports:
    /// `named_imports` stores the original name `A`, while the file exports
    /// the local alias `B`. The name lookup therefore treats `B` as a local
    /// definition, matching the Phase 2 shadow-silencing logic.
    ///
    /// Origin resolution:
    /// 1. `export_locations[name]` with a valid (non-self, lines.start > 0)
    ///    entry — first choice.
    /// 2. Fallback to `(file, import_line, import_line)` using the
    ///    re-exporter's own `export_lines[i]` when the origin is not in the
    ///    index (e.g. imported from a third-party package outside the
    ///    workspace). The entry is still actionable — agents can jump to
    ///    the import line to see where it comes from.
    ///
    /// Results are sorted alphabetically by name for stable output.
    pub fn reexports_in_file(&self, file: &str) -> Vec<OutlineReExport> {
        let Some(entry) = self.files.get(file) else {
            return Vec::new();
        };

        let imported_names: HashSet<&str> = entry
            .named_imports
            .values()
            .flat_map(|v| v.iter().map(String::as_str))
            .collect();

        let mut out = Vec::with_capacity(entry.exports.len());
        for (i, name) in entry.exports.iter().enumerate() {
            if !imported_names.contains(name.as_str()) {
                continue;
            }

            // Prefer the indexed origin definition when available.
            let origin = self
                .export_locations
                .get(name)
                .filter(|loc| loc.file != file)
                .and_then(|loc| {
                    let lines = loc.lines.as_ref()?;
                    if lines.start == 0 {
                        return None;
                    }
                    Some((loc.file.clone(), lines.start, lines.end))
                });

            let (origin_file, origin_start, origin_end) = match origin {
                Some(r) => r,
                None => {
                    // Fall back to the re-exporter's own import line.
                    let (s, e) = entry
                        .export_lines
                        .as_ref()
                        .and_then(|els| els.get(i))
                        .filter(|el| el.start > 0)
                        .map(|el| (el.start, el.end))
                        .unwrap_or((0, 0));
                    (file.to_string(), s, e)
                }
            };

            out.push(OutlineReExport {
                name: name.clone(),
                origin_file,
                origin_start,
                origin_end,
            });
        }

        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
