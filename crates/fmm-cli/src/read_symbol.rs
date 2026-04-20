use fmm_core::manifest::{ExportLines, Manifest};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct ReadSymbolResult {
    pub(crate) symbol: String,
    pub(crate) file: String,
    pub(crate) lines: ExportLines,
    pub(crate) content: ReadSymbolContent,
}

#[derive(Debug, Clone)]
pub(crate) enum ReadSymbolContent {
    Source(String),
    ClassRedirect { methods: Vec<ReadMethodHint> },
}

#[derive(Debug, Clone)]
pub(crate) struct ReadMethodHint {
    pub(crate) name: String,
    pub(crate) lines: ExportLines,
}

impl ReadSymbolResult {
    pub(crate) fn format_text(&self, line_numbers: bool) -> String {
        match &self.content {
            ReadSymbolContent::Source(source) => fmm_core::format::format_read_symbol(
                &self.symbol,
                &self.file,
                &self.lines,
                source,
                line_numbers,
            ),
            ReadSymbolContent::ClassRedirect { methods } => {
                let method_refs: Vec<(&str, &ExportLines)> = methods
                    .iter()
                    .map(|method| (method.name.as_str(), &method.lines))
                    .collect();
                fmm_core::format::format_class_redirect(
                    &self.symbol,
                    &self.file,
                    &self.lines,
                    &method_refs,
                )
            }
        }
    }
}

pub(crate) fn read_symbol_result(
    manifest: &Manifest,
    root: &Path,
    name: &str,
    truncate: bool,
) -> Result<ReadSymbolResult, String> {
    if name.trim().is_empty() {
        return Err(
            "Symbol name must not be empty. Use fmm_list_exports to discover available symbols."
                .to_string(),
        );
    }

    let (resolved_file, resolved_lines) = resolve_symbol_location(manifest, root, name)?;
    let lines = resolved_lines.ok_or_else(|| {
        format!(
            "No line range for '{}' in '{}'. Run 'fmm generate' to re-index.",
            name, resolved_file,
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

    let is_bare_name = !name.contains('.') && !name.contains(':');
    if is_bare_name
        && truncate
        && symbol_source.len() > crate::mcp::MAX_RESPONSE_BYTES
        && let Some(methods) = class_method_hints(manifest, &resolved_file, name)
        && !methods.is_empty()
    {
        return Ok(ReadSymbolResult {
            symbol: name.to_string(),
            file: resolved_file,
            lines,
            content: ReadSymbolContent::ClassRedirect { methods },
        });
    }

    Ok(ReadSymbolResult {
        symbol: name.to_string(),
        file: resolved_file,
        lines,
        content: ReadSymbolContent::Source(symbol_source),
    })
}

fn resolve_symbol_location(
    manifest: &Manifest,
    root: &Path,
    name: &str,
) -> Result<(String, Option<ExportLines>), String> {
    if let Some(colon_pos) = name.find(':') {
        resolve_colon_notation(root, name, colon_pos)
    } else if name.contains('.') {
        resolve_dotted_notation(manifest, root, name)
    } else {
        resolve_export(manifest, name)
    }
}

fn resolve_colon_notation(
    root: &Path,
    name: &str,
    colon_pos: usize,
) -> Result<(String, Option<ExportLines>), String> {
    let file_part = &name[..colon_pos];
    let symbol_part = &name[colon_pos + 1..];

    if (file_part.contains('/') || file_part.contains('.')) && !symbol_part.is_empty() {
        let (start, end) =
            fmm_core::manifest::private_members::find_top_level_function_range(
                root,
                file_part,
                symbol_part,
            )
            .ok_or_else(|| {
                format!(
                    "Symbol '{}' not found in '{}'. Exported symbols must be read by plain name. Use fmm_file_outline(file: \"{}\", include_private: true) to see all top-level declarations.",
                    symbol_part, file_part, file_part
                )
            })?;
        Ok((
            file_part.to_string(),
            Some(fmm_core::manifest::ExportLines { start, end }),
        ))
    } else {
        Err(format!(
            "Ambiguous name '{}'. For file:symbol notation, the file path must contain '/' or '.' (e.g. 'src/helpers.ts:myFn').",
            name
        ))
    }
}

fn resolve_dotted_notation(
    manifest: &Manifest,
    root: &Path,
    name: &str,
) -> Result<(String, Option<ExportLines>), String> {
    if let Some(loc) = manifest.method_index.get(name) {
        return Ok((loc.file.clone(), loc.lines.clone()));
    }

    let dot = name.rfind('.').expect("name contains dot");
    let class_name = &name[..dot];
    let method_name = &name[dot + 1..];

    let class_file = manifest
        .export_locations
        .get(class_name)
        .map(|loc| loc.file.clone())
        .ok_or_else(|| {
            format!(
                "Method '{}' not found. Class '{}' is not a known export. Use fmm_file_outline to inspect the file.",
                name, class_name
            )
        })?;

    let (start, end) = fmm_core::manifest::private_members::find_private_method_range(
        root,
        &class_file,
        class_name,
        method_name,
    )
    .ok_or_else(|| {
        format!(
            "Method '{}' not found. '{}' is not a public or private method of '{}'. Use fmm_file_outline(include_private: true) to see all members.",
            name, method_name, class_name
        )
    })?;

    Ok((
        class_file,
        Some(fmm_core::manifest::ExportLines { start, end }),
    ))
}

fn resolve_export(
    manifest: &Manifest,
    name: &str,
) -> Result<(String, Option<ExportLines>), String> {
    let location = manifest.export_locations.get(name).ok_or_else(|| {
        format!(
            "Export '{}' not found. Use fmm_list_exports or fmm_search to discover available symbols.",
            name
        )
    })?;

    if crate::mcp::tools::is_reexport_file(&location.file)
        && let Some((concrete_file, concrete_lines)) =
            crate::mcp::tools::find_concrete_definition(manifest, name, &location.file)
    {
        Ok((concrete_file, Some(concrete_lines)))
    } else {
        Ok((location.file.clone(), location.lines.clone()))
    }
}

fn class_method_hints(
    manifest: &Manifest,
    resolved_file: &str,
    class_name: &str,
) -> Option<Vec<ReadMethodHint>> {
    let file_entry = manifest.files.get(resolved_file)?;
    let prefix = format!("{class_name}.");
    let mut methods: Vec<ReadMethodHint> = file_entry
        .methods
        .as_ref()?
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix))
        .map(|(key, lines)| ReadMethodHint {
            name: key.trim_start_matches(&prefix).to_string(),
            lines: lines.clone(),
        })
        .collect();

    methods.sort_by_key(|method| method.lines.start);
    Some(methods)
}
