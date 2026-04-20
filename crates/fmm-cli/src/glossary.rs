use fmm_core::manifest::{ExportLines, GlossaryEntry, GlossaryMode, Manifest};
use serde::Serialize;

pub(crate) const DEFAULT_LIMIT: usize = 10;
pub(crate) const HARD_CAP: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlossaryPrecision {
    Named,
    CallSite,
}

impl GlossaryPrecision {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "named" => Ok(Self::Named),
            "call-site" => Ok(Self::CallSite),
            other => Err(format!(
                "invalid precision '{}'; expected 'named' or 'call-site'",
                other
            )),
        }
    }
}

pub(crate) fn parse_mode(value: &str) -> GlossaryMode {
    match value {
        "tests" => GlossaryMode::Tests,
        "all" => GlossaryMode::All,
        _ => GlossaryMode::Source,
    }
}

pub(crate) struct GlossaryQuery<'a> {
    pub(crate) pattern: &'a str,
    pub(crate) mode: GlossaryMode,
    pub(crate) limit: Option<usize>,
    pub(crate) precision: GlossaryPrecision,
}

pub(crate) struct GlossaryResult {
    pub(crate) pattern: String,
    pub(crate) entries: Vec<GlossaryEntry>,
    pub(crate) total_matched: usize,
    pub(crate) limit: usize,
    contextual_message: Option<String>,
    nudge: Option<String>,
}

pub(crate) fn compute_glossary(
    manifest: &Manifest,
    root: &std::path::Path,
    query: GlossaryQuery<'_>,
) -> Result<GlossaryResult, String> {
    let pattern = query.pattern.trim();
    if pattern.is_empty() {
        return Err(
            "pattern is required; provide a symbol name or substring (e.g. 'run_dispatch', 'config'). \
            A full unfiltered glossary on a large codebase would exceed any useful context window."
                .to_string(),
        );
    }

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(HARD_CAP);
    let all_entries = manifest.build_glossary(pattern, query.mode);
    let total_matched = all_entries.len();
    let mut entries: Vec<_> = all_entries.into_iter().take(limit).collect();
    let mut contextual_message = None;

    if let Some(dot_pos) = pattern.rfind('.') {
        let method_name = &pattern[dot_pos + 1..];
        if !method_name.is_empty() {
            let pre_counts: Vec<Vec<usize>> = entries
                .iter()
                .map(|e| e.sources.iter().map(|s| s.used_by.len()).collect())
                .collect();

            for entry in &mut entries {
                for source in &mut entry.sources {
                    source.used_by = fmm_core::manifest::call_site_finder::find_call_sites(
                        root,
                        method_name,
                        &source.used_by,
                    );
                }
            }

            if !entries.is_empty()
                && entries
                    .iter()
                    .all(|e| e.sources.iter().all(|s| s.used_by.is_empty()))
            {
                contextual_message = Some(format_empty_call_site_message(
                    manifest,
                    &entries,
                    &pre_counts,
                    query.mode,
                    method_name,
                ));
            }
        }
    }

    if !pattern.contains('.')
        && !entries.is_empty()
        && manifest.function_index.contains_key(pattern)
    {
        for entry in &mut entries {
            for source in &mut entry.sources {
                apply_named_precision(manifest, root, pattern, source);

                if query.precision == GlossaryPrecision::CallSite {
                    apply_call_site_precision(manifest, root, pattern, source);
                }
            }
        }
    }

    let nudge = build_file_level_nudge(pattern, &entries);

    Ok(GlossaryResult {
        pattern: pattern.to_string(),
        entries,
        total_matched,
        limit,
        contextual_message,
        nudge,
    })
}

pub(crate) fn format_text(result: &GlossaryResult) -> String {
    if let Some(message) = &result.contextual_message {
        return message.clone();
    }

    let mut out = fmm_core::format::format_glossary(
        &result.entries,
        result.total_matched,
        result.limit,
        &result.pattern,
    );
    if let Some(nudge) = &result.nudge {
        out.push_str(nudge);
    }
    out
}

pub(crate) fn json_entries(entries: &[GlossaryEntry]) -> Vec<GlossaryJsonEntry> {
    entries.iter().map(GlossaryJsonEntry::from).collect()
}

fn format_empty_call_site_message(
    manifest: &Manifest,
    entries: &[GlossaryEntry],
    pre_counts: &[Vec<usize>],
    mode: GlossaryMode,
    method_name: &str,
) -> String {
    let mode_label = match mode {
        GlossaryMode::Tests => "test",
        GlossaryMode::All => "all",
        GlossaryMode::Source => "source",
    };
    let mut lines = vec!["---".to_string()];
    for (entry, src_counts) in entries.iter().zip(pre_counts.iter()) {
        lines.push(format!("{}:", fmm_core::format::yaml_escape(&entry.name)));
        for (source, &importer_count) in entry.sources.iter().zip(src_counts.iter()) {
            let basename = source.file.rsplit('/').next().unwrap_or(&source.file);
            lines.push(format!("  (no external {} callers)", mode_label));
            lines.push(format!(
                "  # {} {} import {}, none call {} directly",
                importer_count,
                if importer_count == 1 { "file" } else { "files" },
                basename,
                method_name
            ));
            if matches!(mode, GlossaryMode::Source) {
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
    lines.join("\n")
}

fn apply_named_precision(
    manifest: &Manifest,
    root: &std::path::Path,
    pattern: &str,
    source: &mut fmm_core::manifest::GlossarySource,
) {
    let source_file = source.file.clone();
    let mut named_callers = Vec::new();
    let mut namespace_callers = Vec::new();
    let mut excluded_count = 0usize;

    for candidate in source.used_by.drain(..) {
        match manifest.files.get(&candidate) {
            None => named_callers.push(candidate),
            Some(file_entry) => {
                if file_entry.named_imports.is_empty() && file_entry.namespace_imports.is_empty() {
                    named_callers.push(candidate);
                    continue;
                }

                let specifiers = crate::mcp::tools::compute_import_specifiers(
                    &candidate,
                    &source_file,
                    &manifest.workspace_roots,
                    root,
                );

                if specifiers
                    .iter()
                    .any(|s| file_entry.namespace_imports.contains(s))
                {
                    namespace_callers.push(candidate);
                } else if specifiers.iter().any(|s| {
                    file_entry
                        .named_imports
                        .get(s)
                        .map(|names| names.iter().any(|n| n == pattern))
                        .unwrap_or(false)
                }) {
                    named_callers.push(candidate);
                } else {
                    excluded_count += 1;
                }
            }
        }
    }

    source.used_by = named_callers;
    source.layer2_excluded_count = excluded_count;
    source.layer2_namespace_callers = namespace_callers;
}

fn apply_call_site_precision(
    manifest: &Manifest,
    root: &std::path::Path,
    pattern: &str,
    source: &mut fmm_core::manifest::GlossarySource,
) {
    let layer2_survivors = source.used_by.clone();
    let (confirmed, namespace_callers) =
        fmm_core::manifest::call_site_finder::find_bare_function_callers(
            root,
            pattern,
            &layer2_survivors,
        );

    let confirmed_set: std::collections::HashSet<&str> =
        confirmed.iter().map(|s| s.as_str()).collect();
    let namespace_set: std::collections::HashSet<&str> =
        namespace_callers.iter().map(|(s, _)| s.as_str()).collect();
    let reexport_files: Vec<String> = layer2_survivors
        .iter()
        .filter(|c| !confirmed_set.contains(c.as_str()) && !namespace_set.contains(c.as_str()))
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
    source.namespace_callers = namespace_callers;
    source.reexport_files = reexport_files;
}

fn build_file_level_nudge(pattern: &str, entries: &[GlossaryEntry]) -> Option<String> {
    if pattern.contains('.') || entries.is_empty() {
        return None;
    }

    entries.iter().find(|e| e.name.contains('.')).map(|entry| {
        let total_importers: usize = entry.sources.iter().map(|s| s.used_by.len()).sum();
        let basename = entry
            .sources
            .first()
            .map(|s| s.file.rsplit('/').next().unwrap_or(&s.file))
            .unwrap_or("the file");
        format!(
            "\n# Showing file-level importers ({} {} import {}).\n# For call-site precision: pattern \"{}\"",
            total_importers,
            if total_importers == 1 { "file" } else { "files" },
            basename,
            entry.name
        )
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct GlossaryJsonEntry {
    name: String,
    sources: Vec<GlossaryJsonSource>,
}

impl From<&GlossaryEntry> for GlossaryJsonEntry {
    fn from(entry: &GlossaryEntry) -> Self {
        Self {
            name: entry.name.clone(),
            sources: entry.sources.iter().map(GlossaryJsonSource::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct GlossaryJsonSource {
    file: String,
    lines: Option<ExportLines>,
    used_by: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    namespace_callers: Vec<NamespaceCaller>,
    #[serde(skip_serializing_if = "is_zero")]
    layer2_excluded_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    layer2_namespace_callers: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reexport_files: Vec<String>,
}

impl From<&fmm_core::manifest::GlossarySource> for GlossaryJsonSource {
    fn from(source: &fmm_core::manifest::GlossarySource) -> Self {
        Self {
            file: source.file.clone(),
            lines: source.lines.clone(),
            used_by: source.used_by.clone(),
            namespace_callers: source
                .namespace_callers
                .iter()
                .map(|(file, namespace)| NamespaceCaller {
                    file: file.clone(),
                    namespace: namespace.clone(),
                })
                .collect(),
            layer2_excluded_count: source.layer2_excluded_count,
            layer2_namespace_callers: source.layer2_namespace_callers.clone(),
            reexport_files: source.reexport_files.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct NamespaceCaller {
    file: String,
    namespace: String,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}
