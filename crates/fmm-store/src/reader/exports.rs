use anyhow::Result;
use fmm_core::manifest::{ExportLines, ExportLocation, Manifest, SymbolMetadata};
use rusqlite::Connection;
use std::collections::HashMap;

pub(super) fn load_exports(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT name, file_path, start_line, end_line, signature, visibility, declaration_kind
         FROM exports
         ORDER BY file_path, name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            SymbolMetadata {
                signature: row.get::<_, Option<String>>(4)?,
                visibility: row.get::<_, Option<String>>(5)?,
                declaration_kind: row.get::<_, Option<String>>(6)?,
            },
        ))
    })?;

    // Collect by file so FileEntry.exports + export_lines are built together.
    let mut by_file: HashMap<String, Vec<(String, Option<ExportLines>, SymbolMetadata)>> =
        HashMap::new();

    for row in rows {
        let (name, file_path, start, end, metadata) = row?;
        let lines = match (start, end) {
            (Some(s), Some(e)) if s > 0 => Some(ExportLines {
                start: s as usize,
                end: e as usize,
            }),
            _ => None,
        };
        by_file
            .entry(file_path)
            .or_default()
            .push((name, lines, metadata));
    }

    // Iterate in deterministic order so shadow warnings and index-building
    // are reproducible across runs. HashMap iteration is intentionally randomized.
    #[allow(clippy::type_complexity)]
    let mut sorted: Vec<(String, Vec<(String, Option<ExportLines>, SymbolMetadata)>)> =
        by_file.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for (file_path, entries) in sorted {
        let mut names: Vec<String> = Vec::with_capacity(entries.len());
        let mut line_ranges: Vec<ExportLines> = Vec::with_capacity(entries.len());
        let mut metadata_by_name = HashMap::new();
        let mut has_lines = false;

        for (name, lines, metadata) in &entries {
            names.push(name.clone());
            let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });
            if el.start > 0 {
                has_lines = true;
            }
            line_ranges.push(el);
            metadata_by_name.insert(name.clone(), metadata.clone());
        }

        if let Some(entry) = manifest.files.get_mut(&file_path) {
            entry.exports = names.clone();
            entry.export_metadata = metadata_by_name;
            if has_lines {
                entry.export_lines = Some(line_ranges.clone());
            }
        }

        build_export_indexes(manifest, &file_path, &entries, &line_ranges, has_lines);
    }

    Ok(())
}

fn build_export_indexes(
    manifest: &mut Manifest,
    file_path: &str,
    entries: &[(String, Option<ExportLines>, SymbolMetadata)],
    line_ranges: &[ExportLines],
    has_lines: bool,
) {
    for (i, (name, _lines, _metadata)) in entries.iter().enumerate() {
        let line_range = if has_lines {
            line_ranges
                .get(i)
                .and_then(|l| if l.start > 0 { Some(l.clone()) } else { None })
        } else {
            None
        };

        manifest
            .export_all
            .entry(name.clone())
            .or_default()
            .push(ExportLocation {
                file: file_path.to_string(),
                lines: line_range.clone(),
            });

        if let Some(fe) = manifest.files.get(file_path)
            && fe.function_names.contains(name)
        {
            manifest
                .function_index
                .entry(name.clone())
                .or_insert(ExportLocation {
                    file: file_path.to_string(),
                    lines: line_range.clone(),
                });
        }

        // Re-exports (`from X import Y` + `__all__ = [Y]`) must not claim the
        // `export_index` slot — the original definition already owns it.
        // Detection: the name appears as a value in this file's `named_imports`.
        // `extract_named_imports` stores the original name for aliased imports
        // (`from X import A as B` → A), so aliased re-exports fall through and
        // are treated as local binds.
        let is_reexport = manifest
            .files
            .get(file_path)
            .map(|fe| fe.named_imports.values().any(|v| v.contains(name)))
            .unwrap_or(false);
        if is_reexport {
            continue;
        }

        // Shadow is not a linter concern — the full list of definitions for a
        // name lives in `export_all`. The only deterministic insert rule is
        // `.ts` > `.js`: .js must not overwrite .ts within the TS/JS family.
        // Everything else is last-one-wins.
        let should_insert = match manifest.export_index.get(name) {
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
                .insert(name.clone(), file_path.to_string());
            manifest.export_locations.insert(
                name.clone(),
                ExportLocation {
                    file: file_path.to_string(),
                    lines: line_range,
                },
            );
        }
    }
}

pub(super) fn load_methods(conn: &Connection, manifest: &mut Manifest) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT dotted_name, file_path, start_line, end_line, relationship_kind, signature, visibility, declaration_kind FROM methods",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
            SymbolMetadata {
                signature: row.get::<_, Option<String>>(5)?,
                visibility: row.get::<_, Option<String>>(6)?,
                declaration_kind: row.get::<_, Option<String>>(7)?,
            },
        ))
    })?;

    for row in rows {
        let (dotted_name, file_path, start, end, relationship_kind, metadata) = row?;
        let lines = match (start, end) {
            (Some(s), Some(e)) if s > 0 => Some(ExportLines {
                start: s as usize,
                end: e as usize,
            }),
            _ => None,
        };

        let el = lines.clone().unwrap_or(ExportLines { start: 0, end: 0 });

        if let Some(fe) = manifest.files.get_mut(&file_path) {
            fe.method_metadata
                .insert(dotted_name.clone(), metadata.clone());
            match relationship_kind.as_deref() {
                Some("nested-fn") => {
                    fe.nested_fns.insert(dotted_name.clone(), el);
                }
                Some("closure-state") => {
                    fe.closure_state.insert(dotted_name.clone(), el);
                }
                _ => {
                    fe.methods
                        .get_or_insert_with(HashMap::new)
                        .insert(dotted_name.clone(), el);
                }
            }
        }

        manifest.method_index.insert(
            dotted_name,
            ExportLocation {
                file: file_path,
                lines,
            },
        );
    }

    Ok(())
}
