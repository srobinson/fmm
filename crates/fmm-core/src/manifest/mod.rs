use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::identity::{FileId, FileIdentityMap};
use crate::parser::Metadata;
use crate::resolver::workspace::{WorkspaceEcosystem, WorkspaceInfo};

pub mod call_site_finder;
pub mod private_members;

mod dependency_matcher;
mod file_entry;
mod glossary_builder;
mod reexports;
mod reverse_index;

// Re-export public API consumed by other modules.
pub(crate) use dependency_matcher::{
    builtin_source_extensions, strip_source_ext, try_resolve_local_dep,
};
pub use dependency_matcher::{dep_matches, dotted_dep_matches, python_dep_matches};
pub use file_entry::FileEntry;
pub use glossary_builder::{GlossaryEntry, GlossaryMode, GlossarySource};
pub use reexports::OutlineReExport;
pub use reverse_index::ReverseDeps;

/// Line range for an export symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportLines {
    pub start: usize,
    pub end: usize,
}

/// Export index entry: file path + optional line range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLocation {
    pub file: String,
    pub lines: Option<ExportLines>,
}

/// In-memory index built from the SQLite database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub generated: DateTime<Utc>,
    pub files: HashMap<String, FileEntry>,
    /// Internal file identity map used by graph storage. Skipped at public
    /// boundaries so CLI and MCP contracts remain path based.
    #[serde(skip)]
    file_identity: FileIdentityMap,
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
    pub reverse_deps: ReverseDeps,
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
    /// Workspace packages partitioned by resolver ecosystem.
    /// Not persisted — rebuilt on each load.
    #[serde(skip)]
    pub workspace_packages_by_ecosystem: HashMap<WorkspaceEcosystem, HashMap<String, PathBuf>>,
    /// Workspace roots partitioned by resolver ecosystem.
    /// Not persisted — rebuilt on each load.
    #[serde(skip)]
    pub workspace_roots_by_ecosystem: HashMap<WorkspaceEcosystem, Vec<PathBuf>>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            version: "2.0".to_string(),
            generated: Utc::now(),
            files: HashMap::new(),
            file_identity: FileIdentityMap::default(),
            export_index: HashMap::new(),
            export_locations: HashMap::new(),
            export_all: HashMap::new(),
            method_index: HashMap::new(),
            reverse_deps: HashMap::new(),
            function_index: HashMap::new(),
            workspace_packages: HashMap::new(),
            workspace_roots: Vec::new(),
            workspace_packages_by_ecosystem: HashMap::new(),
            workspace_roots_by_ecosystem: HashMap::new(),
        }
    }

    pub fn set_workspace_info(&mut self, info: WorkspaceInfo) {
        self.workspace_packages = info.packages;
        self.workspace_roots = info.roots;
        self.workspace_packages_by_ecosystem = info.packages_by_ecosystem;
        self.workspace_roots_by_ecosystem = info.roots_by_ecosystem;
    }

    pub fn workspace_packages_for(
        &self,
        ecosystem: WorkspaceEcosystem,
    ) -> &HashMap<String, PathBuf> {
        if let Some(packages) = self.workspace_packages_by_ecosystem.get(&ecosystem) {
            packages
        } else if self.workspace_packages_by_ecosystem.is_empty() {
            &self.workspace_packages
        } else {
            empty_workspace_packages()
        }
    }

    pub fn workspace_roots_for(&self, ecosystem: WorkspaceEcosystem) -> &[PathBuf] {
        if let Some(roots) = self.workspace_roots_by_ecosystem.get(&ecosystem) {
            roots
        } else if self.workspace_roots_by_ecosystem.is_empty() {
            &self.workspace_roots
        } else {
            &[]
        }
    }

    // NOTE: `load()` and `load_from_sqlite()` live in fmm-cli as extension
    // methods on Manifest because they depend on the db module (rusqlite).
    // See ALP-1485 for the full decoupling via FmmStore trait.

    /// Add or update a file entry in the index
    pub fn add_file(&mut self, path: &str, metadata: Metadata) {
        let _ = self.file_identity.ensure_relative_path(path);

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

            // Shadow is not a linter concern — the full list of definitions for
            // a name lives in `export_all`; consumers that care about
            // collisions query that. The only deterministic insert rule is
            // `.ts` > `.js`: .js must not overwrite .ts within the TS/JS
            // family. Everything else is last-one-wins.
            let should_insert = match self.export_index.get(&export_entry.name) {
                None => true,
                Some(existing) if existing == path => true,
                Some(existing) => {
                    let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                    let new_is_js = path.ends_with(".js") || path.ends_with(".jsx");
                    !(existing_is_ts && new_is_js)
                }
            };
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
            let _ = self.file_identity.remove_relative_path(path);

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

    pub fn rebuild_file_identity(&mut self) -> crate::identity::Result<()> {
        self.file_identity =
            FileIdentityMap::from_relative_paths(self.files.keys().map(String::as_str))?;
        Ok(())
    }

    pub fn file_identity(&self) -> &FileIdentityMap {
        &self.file_identity
    }

    pub fn file_id(&self, path: &str) -> Option<FileId> {
        self.file_identity.id_for_path(path)
    }

    pub fn path_for_file_id(&self, id: FileId) -> Option<&str> {
        self.file_identity.path_for_id(id).map(|path| path.as_str())
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
}

fn empty_workspace_packages() -> &'static HashMap<String, PathBuf> {
    static EMPTY: LazyLock<HashMap<String, PathBuf>> = LazyLock::new(HashMap::new);
    &EMPTY
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
