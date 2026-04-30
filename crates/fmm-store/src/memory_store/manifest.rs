use std::collections::HashMap;

use fmm_core::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};

use super::state::InnerState;

/// Build a `Manifest` from stored data, replicating the reader's index logic.
pub(super) fn build_manifest(state: &InnerState) -> Manifest {
    let mut manifest = Manifest::new();

    for (path, sf) in &state.files {
        manifest.files.insert(
            path.clone(),
            FileEntry {
                exports: Vec::new(),
                export_lines: None,
                methods: None,
                imports: sf.imports.clone(),
                dependencies: sf.dependencies.clone(),
                dependency_kinds: sf.dependency_kinds.clone(),
                loc: sf.loc as usize,
                modified: sf.mtime.clone(),
                function_names: sf.function_names.clone(),
                named_imports: sf.named_imports.clone(),
                namespace_imports: sf.namespace_imports.clone(),
                ..Default::default()
            },
        );
    }

    for (file_path, sf) in &state.files {
        populate_exports(&mut manifest, file_path, sf);
    }

    for (file_path, sf) in &state.files {
        populate_methods(&mut manifest, file_path, sf);
    }

    manifest.reverse_deps = state.reverse_deps.clone();

    for (name, path) in &state.workspace_packages {
        manifest.workspace_roots.push(path.clone());
        manifest
            .workspace_packages
            .insert(name.clone(), path.clone());
    }

    manifest
        .rebuild_file_identity()
        .expect("stored file paths must be normalized relative paths");

    manifest
}

fn populate_exports(manifest: &mut Manifest, file_path: &str, sf: &super::state::StoredFile) {
    let mut names: Vec<String> = Vec::with_capacity(sf.exports.len());
    let mut line_ranges: Vec<ExportLines> = Vec::with_capacity(sf.exports.len());
    let mut has_lines = false;

    for exp in &sf.exports {
        names.push(exp.name.clone());
        let el = if exp.start_line > 0 {
            has_lines = true;
            ExportLines {
                start: exp.start_line as usize,
                end: exp.end_line as usize,
            }
        } else {
            ExportLines { start: 0, end: 0 }
        };
        line_ranges.push(el);
    }

    if let Some(entry) = manifest.files.get_mut(file_path) {
        entry.exports = names;
        if has_lines {
            entry.export_lines = Some(line_ranges.clone());
        }
    }

    for (i, exp) in sf.exports.iter().enumerate() {
        let line_range = if has_lines {
            line_ranges
                .get(i)
                .and_then(|l| if l.start > 0 { Some(l.clone()) } else { None })
        } else {
            None
        };

        manifest
            .export_all
            .entry(exp.name.clone())
            .or_default()
            .push(ExportLocation {
                file: file_path.to_string(),
                lines: line_range.clone(),
            });

        if let Some(fe) = manifest.files.get(file_path)
            && fe.function_names.contains(&exp.name)
        {
            manifest
                .function_index
                .entry(exp.name.clone())
                .or_insert(ExportLocation {
                    file: file_path.to_string(),
                    lines: line_range.clone(),
                });
        }

        // TS > JS collision: .js must not overwrite .ts within the TS/JS family;
        // everything else is last-one-wins. Diverges from the SQLite reader by
        // intentionally not skipping re-exports — tests for the in-memory store
        // do not exercise re-export shadow paths.
        let should_insert = match manifest.export_index.get(&exp.name) {
            None => true,
            Some(existing) if existing == file_path => true,
            Some(existing) => {
                let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
                let new_is_js = file_path.ends_with(".js") || file_path.ends_with(".jsx");
                !(existing_is_ts && new_is_js)
            }
        };

        if should_insert {
            manifest
                .export_index
                .insert(exp.name.clone(), file_path.to_string());
            manifest.export_locations.insert(
                exp.name.clone(),
                ExportLocation {
                    file: file_path.to_string(),
                    lines: line_range,
                },
            );
        }
    }
}

fn populate_methods(manifest: &mut Manifest, file_path: &str, sf: &super::state::StoredFile) {
    for method in &sf.methods {
        let lines = if method.start_line > 0 {
            Some(ExportLines {
                start: method.start_line as usize,
                end: method.end_line as usize,
            })
        } else {
            None
        };

        let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });

        if let Some(fe) = manifest.files.get_mut(file_path) {
            match method.kind.as_deref() {
                Some("nested-fn") => {
                    fe.nested_fns.insert(method.dotted_name.clone(), el);
                }
                Some("closure-state") => {
                    fe.closure_state.insert(method.dotted_name.clone(), el);
                }
                _ => {
                    fe.methods
                        .get_or_insert_with(HashMap::new)
                        .insert(method.dotted_name.clone(), el);
                }
            }
        }

        manifest.method_index.insert(
            method.dotted_name.clone(),
            ExportLocation {
                file: file_path.to_string(),
                lines,
            },
        );
    }
}
