//! `fmm_glossary` tool implementation.

use crate::manifest::Manifest;
use crate::mcp::args::GlossaryArgs;
use serde_json::Value;

use super::common::compute_import_specifiers;

pub(in crate::mcp) fn tool_glossary(
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
                    lines.push(format!("{}:", crate::format::yaml_escape(&entry.name)));
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
    if !pattern.contains('.')
        && !entries.is_empty()
        && let Some(_fn_loc) = manifest.function_index.get(pattern)
    {
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
                            let specifiers = compute_import_specifiers(
                                &candidate,
                                &source_file,
                                &manifest.workspace_roots,
                                root,
                            );

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

    let mut out = crate::format::format_glossary(&entries, total_matched, limit, pattern);
    if let Some(n) = nudge {
        out.push_str(&n);
    }
    Ok(out)
}
