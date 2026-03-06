use crate::manifest::Manifest;
use serde_json::Value;

use super::args::{
    DependencyGraphArgs, FileOutlineArgs, GlossaryArgs, ListExportsArgs, ListFilesArgs,
    LookupExportArgs, ReadSymbolArgs, SearchArgs,
};

pub(super) fn tool_lookup_export(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: LookupExportArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    // Try export_locations first, then export_index for backward compat,
    // then method_index for dotted names like "ClassName.method".
    let (file, symbol_lines) = if let Some(loc) = manifest.export_locations.get(&args.name) {
        (loc.file.clone(), loc.lines.clone())
    } else if let Some(file_path) = manifest.export_index.get(&args.name) {
        (file_path.clone(), None)
    } else if let Some(loc) = manifest.method_index.get(&args.name) {
        (loc.file.clone(), loc.lines.clone())
    } else {
        return Err(format!("Export '{}' not found", args.name));
    };

    let entry = manifest
        .files
        .get(&file)
        .ok_or_else(|| format!("File '{}' not found in manifest", file))?;

    // Check export_all for additional definitions (collision detection).
    let collision_note = if let Some(all) = manifest.export_all.get(&args.name) {
        let others: Vec<&str> = all
            .iter()
            .map(|loc| loc.file.as_str())
            .filter(|f| *f != file.as_str())
            .collect();
        if others.is_empty() {
            None
        } else {
            let file_list = others.join(", ");
            Some(format!(
                "⚠ {} additional definition(s) found: [{}] — use fmm_glossary for full collision analysis",
                others.len(),
                file_list
            ))
        }
    } else {
        None
    };

    Ok(crate::format::format_lookup_export(
        &args.name,
        &file,
        symbol_lines.as_ref(),
        entry,
        collision_note.as_deref(),
    ))
}

pub(super) fn tool_list_exports(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    const DEFAULT_LIMIT: usize = 200;

    let args: ListExportsArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let dir = args.directory.as_deref();
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);

    if let Some(ref file_path) = args.file {
        let entry = manifest
            .files
            .get(file_path)
            .ok_or_else(|| format!("File '{}' not found in manifest", file_path))?;
        Ok(crate::format::format_list_exports_file(file_path, entry))
    } else if let Some(ref pat) = args.pattern {
        // Auto-detect regex: if the pattern contains any metacharacter, compile
        // it as a case-sensitive regex.  Plain patterns keep the existing
        // case-insensitive substring match so existing callers are unaffected.
        const METACHAR: &[char] = &['^', '$', '[', '(', '\\', '.', '*', '+', '?', '{'];
        let uses_regex = pat.chars().any(|c| METACHAR.contains(&c));
        let matcher: Box<dyn Fn(&str) -> bool> = if uses_regex {
            match regex::Regex::new(pat) {
                Ok(re) => Box::new(move |name: &str| re.is_match(name)),
                Err(e) => return Err(format!("Invalid pattern: {e}")),
            }
        } else {
            let pat_lower = pat.to_lowercase();
            Box::new(move |name: &str| name.to_lowercase().contains(&pat_lower))
        };

        let mut matches: Vec<(String, String, Option<[usize; 2]>)> = manifest
            .export_index
            .iter()
            .filter(|(name, path)| {
                if let Some(d) = dir {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                matcher(name)
            })
            .map(|(name, path)| {
                let lines = manifest
                    .export_locations
                    .get(name)
                    .and_then(|loc| loc.lines.as_ref())
                    .map(|l| [l.start, l.end]);
                (name.clone(), path.clone(), lines)
            })
            .collect();
        // Also include method_index matches (dotted names like "ClassName.method").
        for (dotted_name, loc) in &manifest.method_index {
            if !matcher(dotted_name) {
                continue;
            }
            if let Some(d) = dir {
                if !loc.file.starts_with(d) {
                    continue;
                }
            }
            let lines = loc.lines.as_ref().map(|l| [l.start, l.end]);
            matches.push((dotted_name.clone(), loc.file.clone(), lines));
        }
        matches.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let total = matches.len();
        let page: Vec<(String, String, Option<[usize; 2]>)> =
            matches.into_iter().skip(offset).take(limit).collect();
        Ok(crate::format::format_list_exports_pattern(
            &page, total, offset,
        ))
    } else {
        let mut by_file: Vec<(&str, &crate::manifest::FileEntry)> = manifest
            .files
            .iter()
            .filter(|(path, entry)| {
                if let Some(d) = dir {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                !entry.exports.is_empty()
            })
            .map(|(path, entry)| (path.as_str(), entry))
            .collect();
        by_file.sort_by_key(|(path, _)| path.to_lowercase());
        let total = by_file.len();
        let page: Vec<(&str, &crate::manifest::FileEntry)> =
            by_file.into_iter().skip(offset).take(limit).collect();
        Ok(crate::format::format_list_exports_all(&page, total, offset))
    }
}

/// Alias for tool_file_outline — delegates entirely for backwards compatibility.
pub(super) fn tool_dependency_graph(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: DependencyGraphArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    validate_not_directory(&args.file, root)?;

    let entry = manifest.files.get(&args.file).ok_or_else(|| {
        format!(
            "File '{}' not found in manifest. Run 'fmm generate' to index the file.",
            args.file
        )
    })?;

    let depth = args.depth.unwrap_or(1);
    let filter = args.filter.as_deref().unwrap_or("all");

    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }

    // Build a predicate that determines whether a file path is kept.
    // Loads config once — same heuristic as fmm_list_files filter.
    let config = crate::config::Config::load_from_dir(root).unwrap_or_default();
    let keep = |path: &str| -> bool {
        match filter {
            "source" => !config.is_test_file(path),
            "tests" => config.is_test_file(path),
            _ => true,
        }
    };

    if depth == 1 {
        // depth=1: use existing single-hop implementation for backward compatibility
        let (local, external, downstream) =
            crate::search::dependency_graph(manifest, &args.file, entry);
        let local: Vec<String> = local.into_iter().filter(|p| keep(p)).collect();
        let downstream: Vec<&String> = downstream
            .into_iter()
            .filter(|p| keep(p.as_str()))
            .collect();
        Ok(crate::format::format_dependency_graph(
            &args.file,
            entry,
            &local,
            &external,
            &downstream,
        ))
    } else {
        // depth>1 or depth=-1: BFS transitive traversal with depth annotations
        let (upstream, external, downstream) =
            crate::search::dependency_graph_transitive(manifest, &args.file, entry, depth);
        let upstream: Vec<(String, i32)> = upstream.into_iter().filter(|(p, _)| keep(p)).collect();
        let downstream: Vec<(String, i32)> =
            downstream.into_iter().filter(|(p, _)| keep(p)).collect();
        Ok(crate::format::format_dependency_graph_transitive(
            &args.file,
            entry,
            &upstream,
            &external,
            &downstream,
            depth,
        ))
    }
}

pub(super) fn tool_read_symbol(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: ReadSymbolArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    if args.name.trim().is_empty() {
        return Err(
            "Symbol name must not be empty. Use fmm_list_exports to discover available symbols."
                .to_string(),
        );
    }

    // Dotted notation: ClassName.method — look up in method_index first.
    // If not found (private method), fall back to on-demand tree-sitter extraction.
    let (resolved_file, resolved_lines) = if args.name.contains('.') {
        if let Some(loc) = manifest.method_index.get(&args.name) {
            (loc.file.clone(), loc.lines.clone())
        } else {
            // ALP-827: private method fallback — parse the file on demand.
            let dot = args.name.rfind('.').unwrap();
            let class_name = &args.name[..dot];
            let method_name = &args.name[dot + 1..];

            let class_file = manifest
                .export_locations
                .get(class_name)
                .map(|loc| loc.file.clone())
                .ok_or_else(|| {
                    format!(
                        "Method '{}' not found. Class '{}' is not a known export. \
                         Use fmm_file_outline to inspect the file.",
                        args.name, class_name
                    )
                })?;

            let (start, end) = crate::manifest::private_members::find_private_method_range(
                root,
                &class_file,
                class_name,
                method_name,
            )
            .ok_or_else(|| {
                format!(
                    "Method '{}' not found. '{}' is not a public or private method of \
                         '{}'. Use fmm_file_outline(include_private: true) to see all members.",
                    args.name, method_name, class_name
                )
            })?;

            (
                class_file,
                Some(crate::manifest::ExportLines { start, end }),
            )
        }
    } else {
        let location = manifest
            .export_locations
            .get(&args.name)
            .ok_or_else(|| format!("Export '{}' not found. Use fmm_list_exports or fmm_search to discover available symbols.", args.name))?;

        // If the winning location is a re-export hub (index file), try to find the
        // concrete definition in a nearby non-index file that also exports this symbol.
        if is_reexport_file(&location.file) {
            if let Some((concrete_file, concrete_lines)) =
                find_concrete_definition(manifest, &args.name, &location.file)
            {
                (concrete_file, Some(concrete_lines))
            } else {
                (location.file.clone(), location.lines.clone())
            }
        } else {
            (location.file.clone(), location.lines.clone())
        }
    };

    let lines = resolved_lines.ok_or_else(|| {
        format!(
            "No line range for '{}' in '{}' — regenerate sidecars with 'fmm generate' for v0.3 format",
            args.name, resolved_file,
        )
    })?;

    let source_path = root.join(&resolved_file);
    let content = std::fs::read_to_string(&source_path)
        .map_err(|e| format!("Cannot read '{}': {}", resolved_file, e))?;

    let source_lines: Vec<&str> = content.lines().collect();
    let start = lines.start.saturating_sub(1);
    let end = lines.end.min(source_lines.len());

    if start >= source_lines.len() {
        return Err(format!(
            "Line range [{}, {}] out of bounds for '{}' ({} lines)",
            lines.start,
            lines.end,
            resolved_file,
            source_lines.len()
        ));
    }

    let symbol_source = source_lines[start..end].join("\n");

    // Bare class redirect: when a bare class name (no dot) would exceed the 10KB cap
    // and truncate was not explicitly disabled, return an outline with redirect hints
    // instead of a misleading partial view of the class body.
    let is_bare_name = !args.name.contains('.');
    let should_truncate = args.truncate.unwrap_or(true);
    if is_bare_name
        && should_truncate
        && symbol_source.len() > crate::mcp::McpServer::MAX_RESPONSE_BYTES
    {
        // Check if this class has methods registered in the file entry.
        if let Some(file_entry) = manifest.files.get(&resolved_file) {
            let prefix = format!("{}.", args.name);
            let mut class_methods: Vec<(&str, &crate::manifest::ExportLines)> = file_entry
                .methods
                .as_ref()
                .map(|m| {
                    m.iter()
                        .filter(|(k, _)| k.starts_with(&prefix))
                        .map(|(k, v)| (k.trim_start_matches(&prefix), v))
                        .collect()
                })
                .unwrap_or_default();
            if !class_methods.is_empty() {
                // Sort by line start order for readability.
                class_methods.sort_by_key(|(_, el)| el.start);
                return Ok(crate::format::format_class_redirect(
                    &args.name,
                    &resolved_file,
                    &lines,
                    &class_methods,
                ));
            }
        }
    }

    Ok(crate::format::format_read_symbol(
        &args.name,
        &resolved_file,
        &lines,
        &symbol_source,
        args.line_numbers.unwrap_or(false),
    ))
}

pub(super) fn tool_file_outline(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: FileOutlineArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    validate_not_directory(&args.file, root)?;

    let entry = manifest.files.get(&args.file).ok_or_else(|| {
        format!(
            "File '{}' not found in manifest. Run 'fmm generate' to index the file.",
            args.file
        )
    })?;

    let include_private = args.include_private.unwrap_or(false);
    let private_by_class = if include_private {
        let class_names: Vec<&str> = entry.exports.iter().map(|s| s.as_str()).collect();
        Some(crate::manifest::private_members::extract_private_members(
            root,
            &args.file,
            &class_names,
        ))
    } else {
        None
    };

    Ok(crate::format::format_file_outline(
        &args.file,
        entry,
        private_by_class.as_ref(),
    ))
}

pub(super) fn tool_list_files(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    const DEFAULT_LIMIT: usize = 200;

    let args: ListFilesArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    // Normalise "." / "./" to None so callers get the full index, matching
    // the behaviour of omitting the directory parameter entirely.
    let dir = args.directory.as_deref().and_then(|d| {
        if matches!(d, "." | "./") {
            None
        } else {
            Some(d)
        }
    });
    let pat = args.pattern.as_deref();
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);
    let sort_by = args.sort_by.as_deref().unwrap_or("loc");
    let order = args.order.as_deref();
    let group_by = args.group_by.as_deref();
    let filter = args.filter.as_deref().unwrap_or("all");

    if !matches!(
        sort_by,
        "name" | "loc" | "exports" | "downstream" | "modified"
    ) {
        return Err(format!(
            "Invalid sort_by '{}'. Valid values: name, loc, exports, downstream, modified.",
            sort_by
        ));
    }
    if let Some(o) = order {
        if !matches!(o, "asc" | "desc") {
            return Err(format!("Invalid order '{}'. Valid values: asc, desc.", o));
        }
    }
    if let Some(g) = group_by {
        if g != "subdir" {
            return Err(format!("Invalid group_by '{}'. Valid values: subdir.", g));
        }
    }
    if !matches!(filter, "all" | "source" | "tests") {
        return Err(format!(
            "Invalid filter '{}'. Valid values: all, source, tests.",
            filter
        ));
    }

    // Load config for test-file detection (used when filter != "all").
    let config = crate::config::Config::load_from_dir(_root).unwrap_or_default();

    let mut entries: Vec<(&str, usize, usize, usize, Option<&str>)> = manifest
        .files
        .iter()
        .filter(|(path, _)| {
            if let Some(d) = dir {
                if !path.starts_with(d) {
                    return false;
                }
            }
            // Apply source/test filter
            match filter {
                "tests" => {
                    if !config.is_test_file(path) {
                        return false;
                    }
                }
                "source" => {
                    if config.is_test_file(path) {
                        return false;
                    }
                }
                _ => {} // "all": no filter
            }
            if let Some(p) = pat {
                let filename = path.rsplit('/').next().unwrap_or(path.as_str());
                if !glob_filename_matches(p, filename) {
                    return false;
                }
            }
            true
        })
        .map(|(path, entry)| {
            let downstream = manifest
                .reverse_deps
                .get(path.as_str())
                .map(|v| v.len())
                .unwrap_or(0);
            let modified = entry.modified.as_deref();
            (
                path.as_str(),
                entry.loc,
                entry.exports.len(),
                downstream,
                modified,
            )
        })
        .collect();

    // Rollup mode: group by immediate subdirectory.
    if group_by == Some("subdir") {
        // Rollup only uses (path, loc, exports) — strip downstream/modified before passing
        let stripped: Vec<(&str, usize, usize)> =
            entries.iter().map(|(p, l, e, _, _)| (*p, *l, *e)).collect();
        return Ok(build_rollup(stripped, dir, sort_by, order));
    }

    // Smart defaults: loc/exports/downstream/modified sort descending; name sorts ascending.
    let desc = match sort_by {
        "loc" | "exports" | "downstream" | "modified" => order != Some("asc"),
        _ => order == Some("desc"),
    };

    match sort_by {
        "loc" => {
            if desc {
                entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, a, _, _, _), (_, b, _, _, _)| a.cmp(b));
            }
        }
        "exports" => {
            if desc {
                entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, a, _, _), (_, _, b, _, _)| a.cmp(b));
            }
        }
        "downstream" => {
            if desc {
                entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, _, a, _), (_, _, _, b, _)| a.cmp(b));
            }
        }
        "modified" => {
            // Lexicographic sort on YYYY-MM-DD strings works correctly for date ordering.
            // Files with no modified date sort last.
            if desc {
                entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| b.cmp(a));
            } else {
                entries.sort_by(|(_, _, _, _, a), (_, _, _, _, b)| a.cmp(b));
            }
        }
        _ => {
            if desc {
                entries.sort_by(|(a, _, _, _, _), (b, _, _, _, _)| {
                    b.to_lowercase().cmp(&a.to_lowercase())
                });
            } else {
                entries.sort_by_key(|(path, _, _, _, _)| path.to_lowercase());
            }
        }
    }

    let total = entries.len();
    let total_loc: usize = entries.iter().map(|(_, loc, _, _, _)| loc).sum();
    let largest = entries
        .iter()
        .max_by_key(|(_, loc, _, _, _)| loc)
        .map(|(path, loc, _, _, _)| (*path, *loc));
    let show_modified = sort_by == "modified";
    let page: Vec<(&str, usize, usize, usize, Option<&str>)> =
        entries.into_iter().skip(offset).take(limit).collect();

    Ok(crate::format::format_list_files(
        dir,
        &page,
        total,
        total_loc,
        largest,
        offset,
        show_modified,
    ))
}

/// Compute directory rollup for group_by="subdir" and format the result.
fn build_rollup(
    entries: Vec<(&str, usize, usize)>,
    prefix: Option<&str>,
    sort_by: &str,
    order: Option<&str>,
) -> String {
    let total_files = entries.len();
    let total_loc: usize = entries.iter().map(|(_, loc, _)| loc).sum();
    let bucket_vec = crate::format::compute_rollup_buckets(&entries, prefix, sort_by, order);
    crate::format::format_list_files_rollup(prefix, &bucket_vec, total_files, total_loc)
}

pub(super) fn tool_search(
    manifest: &Manifest,
    _root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: SearchArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let has_filters = args.export.is_some()
        || args.imports.is_some()
        || args.depends_on.is_some()
        || args.min_loc.is_some()
        || args.max_loc.is_some();

    let term = args.term;
    let limit = args.limit;
    let filters = crate::search::SearchFilters {
        export: args.export,
        imports: args.imports,
        depends_on: args.depends_on,
        min_loc: args.min_loc,
        max_loc: args.max_loc,
    };

    if let Some(term) = term {
        let mut result = crate::search::bare_search(manifest, &term, limit);
        // When structured filters are also present, intersect with AND semantics:
        // keep only exports/files/imports that are in the filter file set.
        if has_filters {
            let filter_results = crate::search::filter_search(manifest, &filters);
            let filter_files: std::collections::HashSet<&str> =
                filter_results.iter().map(|r| r.file.as_str()).collect();
            result
                .exports
                .retain(|h| filter_files.contains(h.file.as_str()));
            result.files.retain(|f| filter_files.contains(f.as_str()));
            result.imports.iter_mut().for_each(|h| {
                h.files.retain(|f| filter_files.contains(f.as_str()));
            });
            result.imports.retain(|h| !h.files.is_empty());
        }
        return Ok(crate::format::format_bare_search(&result, false));
    }

    // Structured filter search (no term)
    let results = crate::search::filter_search(manifest, &filters);
    let total_count = results.len();
    let formatted = crate::format::format_filter_search(&results, false);

    // ALP-861: when depends_on is used, add a count header and transitive/direct clarification.
    if let Some(ref dep_path) = filters.depends_on {
        let limit_note = if total_count > 0 {
            format!(
                "{} file{} depend on {} (transitive — includes indirect dependents).",
                total_count,
                if total_count == 1 { "" } else { "s" },
                dep_path,
            )
        } else {
            format!("0 files depend on {} (transitive).", dep_path)
        };
        let footer = format!(
            "# For direct dependents only: fmm_dependency_graph({})",
            dep_path
        );
        return Ok(format!(
            "# {}

{}

{}",
            limit_note, formatted, footer
        ));
    }

    Ok(formatted)
}

pub(super) fn tool_glossary(
    manifest: &Manifest,
    root: &std::path::Path,
    args: &Value,
) -> Result<String, String> {
    let args: GlossaryArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

    let pattern = args.pattern.as_deref().unwrap_or("").trim();
    if pattern.is_empty() {
        return Err(
            "pattern is required — provide a symbol name or substring (e.g. 'run_dispatch', 'config'). \
            A full unfiltered glossary on a large codebase would exceed any useful context window."
                .to_string(),
        );
    }

    const DEFAULT_LIMIT: usize = 10;
    const HARD_CAP: usize = 50;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).min(HARD_CAP);
    let mode = match args.mode.as_deref().unwrap_or("source") {
        "tests" => crate::manifest::GlossaryMode::Tests,
        "all" => crate::manifest::GlossaryMode::All,
        _ => crate::manifest::GlossaryMode::Source,
    };
    // ALP-883: "named" (default) = Layer 2 only; "call-site" = Layer 2 + Layer 3 tree-sitter.
    let run_layer3 = args.precision.as_deref() == Some("call-site");

    let all_entries = manifest.build_glossary(pattern, mode);
    let total_matched = all_entries.len();
    let mut entries: Vec<_> = all_entries.into_iter().take(limit).collect();

    // ALP-785: For dotted method queries (e.g. "ClassName.method"), refine
    // used_by via tree-sitter call-site detection (pass 2 of 2-pass architecture).
    // Non-dotted queries skip this — file-level used_by is correct for class-level.
    if let Some(dot_pos) = pattern.rfind('.') {
        let method_name = &pattern[dot_pos + 1..];
        if !method_name.is_empty() {
            // ALP-826: capture pre-refinement importer counts for contextual
            // messaging when call-site search returns zero callers.
            let pre_counts: Vec<Vec<usize>> = entries
                .iter()
                .map(|e| e.sources.iter().map(|s| s.used_by.len()).collect())
                .collect();

            for entry in &mut entries {
                for source in &mut entry.sources {
                    let refined = crate::manifest::call_site_finder::find_call_sites(
                        root,
                        method_name,
                        &source.used_by,
                    );
                    source.used_by = refined;
                }
            }

            // ALP-826: when all used_by are empty after refinement, return a
            // contextual message instead of a list of `used_by: []` lines.
            if entries
                .iter()
                .all(|e| e.sources.iter().all(|s| s.used_by.is_empty()))
            {
                let mode_label = match mode {
                    crate::manifest::GlossaryMode::Tests => "test",
                    crate::manifest::GlossaryMode::All => "all",
                    _ => "source",
                };
                let mut lines = vec!["---".to_string()];
                for (entry, src_counts) in entries.iter().zip(pre_counts.iter()) {
                    lines.push(format!("{}:", crate::formatter::yaml_escape(&entry.name)));
                    for (source, &importer_count) in entry.sources.iter().zip(src_counts.iter()) {
                        let basename = source.file.rsplit('/').next().unwrap_or(&source.file);
                        lines.push(format!("  (no external {} callers)", mode_label));
                        lines.push(format!(
                            "  # {} {} import {} — none call {} directly",
                            importer_count,
                            if importer_count == 1 { "file" } else { "files" },
                            basename,
                            method_name
                        ));
                        if matches!(mode, crate::manifest::GlossaryMode::Source) {
                            let test_count = manifest.count_test_dependents(&source.file);
                            if test_count > 0 {
                                lines.push(format!(
                                    "  # {} test {} found (rerun with mode: tests)",
                                    test_count,
                                    if test_count == 1 { "caller" } else { "callers" }
                                ));
                            }
                        }
                    }
                }
                return Ok(lines.join("\n"));
            }
        }
    }

    // ALP-882 + ALP-865: for bare-name queries that match a module-level function declaration,
    // apply Layer 2 (named import filter) then Layer 3 (call-site verification).
    if !pattern.contains('.') && !entries.is_empty() {
        if let Some(_fn_loc) = manifest.function_index.get(pattern) {
            for entry in &mut entries {
                for source in &mut entry.sources {
                    // Layer 2: named import filter — index-only, no tree-sitter.
                    // Shrinks used_by from all module importers (~24) to named-import callers (~10).
                    let source_file = source.file.clone();
                    let mut named_callers: Vec<String> = Vec::new();
                    let mut l2_ns: Vec<String> = Vec::new();
                    let mut l2_excluded: usize = 0;

                    for candidate in source.used_by.drain(..) {
                        match manifest.files.get(&candidate) {
                            None => {
                                // No FileEntry — include to avoid false negatives.
                                named_callers.push(candidate);
                            }
                            Some(fe) => {
                                // If the file has no named_imports data (non-TS/JS or pre-v0.4
                                // sidecar), fall through: include to avoid false negatives.
                                if fe.named_imports.is_empty() && fe.namespace_imports.is_empty() {
                                    named_callers.push(candidate);
                                    continue;
                                }
                                let specifiers =
                                    compute_import_specifiers(&candidate, &source_file);

                                if specifiers.iter().any(|s| fe.namespace_imports.contains(s)) {
                                    l2_ns.push(candidate);
                                } else if specifiers.iter().any(|s| {
                                    fe.named_imports
                                        .get(s)
                                        .map(|names| names.iter().any(|n| n == pattern))
                                        .unwrap_or(false)
                                }) {
                                    named_callers.push(candidate);
                                } else {
                                    l2_excluded += 1;
                                }
                            }
                        }
                    }

                    source.used_by = named_callers;
                    source.layer2_excluded_count = l2_excluded;
                    source.layer2_namespace_callers = l2_ns;

                    // Layer 3: call-site verification (tree-sitter) — opt-in via precision: "call-site".
                    // Removes dead imports and annotates re-exports. Runs on the smaller Layer 2
                    // set, making tree-sitter cheaper on large codebases.
                    if run_layer3 {
                        let l2_survivors = source.used_by.clone();
                        let (confirmed, ns_callers) =
                            crate::manifest::call_site_finder::find_bare_function_callers(
                                root,
                                pattern,
                                &l2_survivors,
                            );

                        // Detect re-exports: files excluded by Layer 3 that also export the symbol.
                        // These are NOT callers but they ARE impacted by a rename.
                        let confirmed_set: std::collections::HashSet<&str> =
                            confirmed.iter().map(|s| s.as_str()).collect();
                        let ns_set: std::collections::HashSet<&str> =
                            ns_callers.iter().map(|(s, _)| s.as_str()).collect();
                        let reexports: Vec<String> = l2_survivors
                            .iter()
                            .filter(|c| {
                                !confirmed_set.contains(c.as_str()) && !ns_set.contains(c.as_str())
                            })
                            .filter(|c| {
                                manifest
                                    .export_all
                                    .get(pattern)
                                    .map(|locs| locs.iter().any(|loc| &loc.file == *c))
                                    .unwrap_or(false)
                            })
                            .cloned()
                            .collect();

                        source.used_by = confirmed;
                        source.namespace_callers = ns_callers;
                        source.reexport_files = reexports;
                    }
                }
            }
        }
    }

    // ALP-826: for bare-name queries, append a nudge when the results include
    // a dotted method-index entry — the used_by list is file-level importers,
    // not confirmed call-site callers, and agents benefit from knowing this.
    let nudge = if !pattern.contains('.') && !entries.is_empty() {
        entries
            .iter()
            .find(|e| e.name.contains('.'))
            .map(|dotted_entry| {
                let total_importers: usize =
                    dotted_entry.sources.iter().map(|s| s.used_by.len()).sum();
                let basename = dotted_entry
                    .sources
                    .first()
                    .map(|s| s.file.rsplit('/').next().unwrap_or(&s.file))
                    .unwrap_or("the file");
                format!(
                    "\n# Showing file-level importers ({} {} import {}).\n# For call-site precision: pattern \"{}\"",
                    total_importers,
                    if total_importers == 1 { "file" } else { "files" },
                    basename,
                    dotted_entry.name
                )
            })
    } else {
        None
    };

    let mut out = crate::format::format_glossary(&entries, total_matched, limit);
    if let Some(n) = nudge {
        out.push_str(&n);
    }
    Ok(out)
}

/// Return true if a file path is a conventional re-export hub (index/init file).
/// These files aggregate symbols from sub-modules and are not the definition site.
pub(crate) fn is_reexport_file(file_path: &str) -> bool {
    let filename = file_path.rsplit('/').next().unwrap_or(file_path);
    matches!(
        filename,
        "__init__.py" | "index.ts" | "index.tsx" | "index.js" | "index.jsx" | "mod.rs"
    )
}

/// Given that `symbol` was found in a re-export hub, search the manifest for a
/// non-index file that also exports the same symbol, preferring files whose
/// directory path shares the most prefix with `reexport_file`.
///
/// Returns `(concrete_file_path, ExportLines)` or `None` if no candidate found.
pub(crate) fn find_concrete_definition(
    manifest: &crate::manifest::Manifest,
    symbol: &str,
    reexport_file: &str,
) -> Option<(String, crate::manifest::ExportLines)> {
    let reexport_dir = reexport_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

    let mut candidates: Vec<(String, crate::manifest::ExportLines, usize)> = manifest
        .files
        .iter()
        .filter(|(path, _)| {
            let p = path.as_str();
            p != reexport_file && !is_reexport_file(p)
        })
        .filter_map(|(path, entry)| {
            // Find this symbol in the file's export list
            let idx = entry.exports.iter().position(|e| e == symbol)?;
            // Require line-range data — without it we cannot show source
            let lines = entry
                .export_lines
                .as_ref()
                .and_then(|el| el.get(idx))
                .filter(|l| l.start > 0)?;
            // Shared prefix length as proximity score
            let file_dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let shared = reexport_dir
                .chars()
                .zip(file_dir.chars())
                .take_while(|(a, b)| a == b)
                .count();
            Some((path.clone(), lines.clone(), shared))
        })
        .collect();

    // Sort by proximity descending so closest sibling wins
    candidates.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));
    candidates.into_iter().map(|(f, l, _)| (f, l)).next()
}

/// Return an error if `path` looks like a directory (ends with `/` or resolves to a dir on disk).
/// Provides a helpful message pointing to fmm_list_files.
pub(super) fn validate_not_directory(path: &str, root: &std::path::Path) -> Result<(), String> {
    if path.ends_with('/') || path.ends_with(std::path::MAIN_SEPARATOR) {
        return Err(format!(
            "'{}' is a directory, not a file. Use fmm_list_files(directory: \"{}\") to list its contents.",
            path, path
        ));
    }
    let resolved = root.join(path);
    if resolved.is_dir() {
        return Err(format!(
            "'{}' is a directory, not a file. Use fmm_list_files(directory: \"{}/\") to list its contents.",
            path, path
        ));
    }
    Ok(())
}

/// Compute the possible import path specifiers that a file at `candidate_path` would use
/// when importing from `source_file`. Both paths are manifest-relative (e.g.
/// `packages/react-reconciler/src/ReactFiberHooks.js`).
///
/// Returns up to two forms: without extension (the common TS/JS convention) and with extension.
/// Same-directory imports get a `./` prefix; cross-directory ones use `../` traversal.
///
/// These specifiers are compared against keys in `FileEntry::named_imports` and
/// `FileEntry::namespace_imports`, which store paths as written in the import statement.
pub(super) fn compute_import_specifiers(candidate_path: &str, source_file: &str) -> Vec<String> {
    let candidate_dir = candidate_path
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("");
    let (source_dir, source_filename) = source_file.rsplit_once('/').unwrap_or(("", source_file));
    let source_base = source_filename
        .rsplit_once('.')
        .map(|(b, _)| b)
        .unwrap_or(source_filename);

    let candidate_segs: Vec<&str> = if candidate_dir.is_empty() {
        vec![]
    } else {
        candidate_dir.split('/').collect()
    };
    let source_segs: Vec<&str> = if source_dir.is_empty() {
        vec![]
    } else {
        source_dir.split('/').collect()
    };

    let common = candidate_segs
        .iter()
        .zip(source_segs.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let up = candidate_segs.len() - common;
    let down = &source_segs[common..];

    let mut parts: Vec<&str> = std::iter::repeat_n("..", up).collect();
    parts.extend(down.iter().copied());

    let (base_specifier, ext_specifier) = if parts.is_empty() {
        (
            format!("./{}", source_base),
            format!("./{}", source_filename),
        )
    } else if up == 0 {
        // Down-only path (source is in a subdirectory of the common ancestor).
        // Parts contain no `..` segments, so we must add `./` — without it the
        // specifier would be treated as a bare package name, not a relative path.
        let suffix = parts.join("/");
        (
            format!("./{}/{}", suffix, source_base),
            format!("./{}/{}", suffix, source_filename),
        )
    } else {
        // Up-then-down path — starts with `..`, already unambiguously relative.
        let prefix = parts.join("/");
        (
            format!("{}/{}", prefix, source_base),
            format!("{}/{}", prefix, source_filename),
        )
    };

    if base_specifier == ext_specifier {
        vec![base_specifier]
    } else {
        vec![base_specifier, ext_specifier]
    }
}

/// Match a glob pattern against a filename (last path component).
/// Supports `*` as a wildcard within the filename. Does not match path separators.
/// Examples: `*.py`, `test_*`, `*_test.rs`, `*`
pub(super) fn glob_filename_matches(pattern: &str, filename: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return filename == pattern;
    }
    // Split on the first `*` and check prefix + suffix
    let (prefix, rest) = pattern.split_once('*').unwrap();
    if !filename.starts_with(prefix) {
        return false;
    }
    let after_prefix = &filename[prefix.len()..];
    // Handle remaining pattern segments (multiple `*`)
    if rest.contains('*') {
        // Recursively match the remainder
        glob_filename_matches(rest, after_prefix)
    } else {
        // Single `*` — remainder is a literal suffix
        after_prefix.ends_with(rest) && after_prefix.len() >= rest.len()
    }
}
