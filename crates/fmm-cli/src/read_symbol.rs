use fmm_core::manifest::{ExportLines, ExportLocation, Manifest};
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

#[derive(Debug, Clone)]
struct SymbolLocation {
    file: String,
    lines: Option<ExportLines>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ReadSymbolGuidance {
    Cli,
    Mcp,
}

impl ReadSymbolGuidance {
    fn empty_symbol(self) -> &'static str {
        match self {
            Self::Cli => {
                "Symbol name must not be empty. Use fmm exports to discover available symbols."
            }
            Self::Mcp => {
                "Symbol name must not be empty. Use fmm_list_exports to discover available symbols."
            }
        }
    }

    fn missing_top_level_function(self, symbol: &str, file: &str) -> String {
        match self {
            Self::Cli => format!(
                "Symbol '{}' not found in '{}'. Exported symbols must be read by plain name. Use fmm outline {} --include-private to see all top-level declarations.",
                symbol, file, file
            ),
            Self::Mcp => format!(
                "Symbol '{}' not found in '{}'. Exported symbols must be read by plain name. Use fmm_file_outline(file: \"{}\", include_private: true) to see all top-level declarations.",
                symbol, file, file
            ),
        }
    }

    fn unknown_class(self, name: &str, class_name: &str) -> String {
        match self {
            Self::Cli => format!(
                "Method '{}' not found. Class '{}' is not a known export. Use fmm exports or fmm search to discover available symbols.",
                name, class_name
            ),
            Self::Mcp => format!(
                "Method '{}' not found. Class '{}' is not a known export. Use fmm_file_outline to inspect the file.",
                name, class_name
            ),
        }
    }

    fn missing_method(self, name: &str, method_name: &str, class_name: &str, file: &str) -> String {
        match self {
            Self::Cli => format!(
                "Method '{}' not found. '{}' is not a public or private method of '{}'. Use fmm outline {} --include-private to see all members.",
                name, method_name, class_name, file
            ),
            Self::Mcp => format!(
                "Method '{}' not found. '{}' is not a public or private method of '{}'. Use fmm_file_outline(include_private: true) to see all members.",
                name, method_name, class_name
            ),
        }
    }

    fn missing_export(self, name: &str) -> String {
        match self {
            Self::Cli => format!(
                "Symbol '{}' not found. Use fmm exports or fmm search to discover available symbols.",
                name
            ),
            Self::Mcp => format!(
                "Symbol '{}' not found. Use fmm_list_exports or fmm_search to discover available symbols.",
                name
            ),
        }
    }

    fn ambiguous_export(self, name: &str, locations: &[SymbolLocation]) -> String {
        let hint = self.file_qualified_hints(locations, name);
        format!(
            "Symbol '{}' is ambiguous: {} indexed exports use this name. Use file-qualified read syntax:\n{}",
            name,
            locations.len(),
            hint
        )
    }

    fn ambiguous_top_level_function(self, name: &str, locations: &[SymbolLocation]) -> String {
        let hint = self.file_qualified_hints(locations, name);
        format!(
            "Symbol '{}' is ambiguous: {} non-exported top-level declarations use this name. Use file-qualified read syntax:\n{}",
            name,
            locations.len(),
            hint
        )
    }

    fn file_qualified_hints(self, locations: &[SymbolLocation], name: &str) -> String {
        locations
            .iter()
            .map(|location| match self {
                Self::Cli => format!("  fmm read {}:{}", location.file, name),
                Self::Mcp => format!("  fmm_read_symbol(name: \"{}:{}\")", location.file, name),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
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
    guidance: ReadSymbolGuidance,
) -> Result<ReadSymbolResult, String> {
    if name.trim().is_empty() {
        return Err(guidance.empty_symbol().to_string());
    }

    let (resolved_file, resolved_lines) = resolve_symbol_location(manifest, root, name, guidance)?;
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
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    if let Some(colon_pos) = name.find(':') {
        resolve_colon_notation(manifest, root, name, colon_pos, guidance)
    } else if name.contains('.') {
        resolve_dotted_notation(manifest, root, name, guidance)
    } else {
        resolve_bare_symbol(manifest, root, name, guidance)
    }
}

fn resolve_colon_notation(
    manifest: &Manifest,
    root: &Path,
    name: &str,
    colon_pos: usize,
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    let file_part = &name[..colon_pos];
    let symbol_part = &name[colon_pos + 1..];

    if !(file_part.contains('/') || file_part.contains('.')) || symbol_part.is_empty() {
        return Err(format!(
            "Ambiguous name '{}'. For file:symbol notation, the file path must contain '/' or '.' (e.g. 'src/helpers.ts:myFn').",
            name
        ));
    }

    if symbol_part.contains('.') {
        return resolve_file_qualified_method(
            manifest,
            root,
            file_part,
            symbol_part,
            name,
            guidance,
        );
    }

    if let Some(lines) = find_export_in_file(manifest, file_part, symbol_part) {
        return Ok((file_part.to_string(), Some(lines)));
    }

    let (start, end) = fmm_core::manifest::private_members::find_top_level_function_range(
        root,
        file_part,
        symbol_part,
    )
    .ok_or_else(|| guidance.missing_top_level_function(symbol_part, file_part))?;

    Ok((
        file_part.to_string(),
        Some(fmm_core::manifest::ExportLines { start, end }),
    ))
}

fn resolve_file_qualified_method(
    manifest: &Manifest,
    root: &Path,
    file_part: &str,
    symbol_part: &str,
    name: &str,
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    if let Some(lines) = manifest
        .files
        .get(file_part)
        .and_then(|entry| entry.methods.as_ref())
        .and_then(|methods| methods.get(symbol_part))
        .cloned()
    {
        return Ok((file_part.to_string(), Some(lines)));
    }

    let dot = symbol_part.rfind('.').expect("symbol part contains dot");
    let class_name = &symbol_part[..dot];
    let method_name = &symbol_part[dot + 1..];

    let (start, end) = fmm_core::manifest::private_members::find_private_method_range(
        root,
        file_part,
        class_name,
        method_name,
    )
    .ok_or_else(|| guidance.missing_method(name, method_name, class_name, file_part))?;

    Ok((
        file_part.to_string(),
        Some(fmm_core::manifest::ExportLines { start, end }),
    ))
}

fn find_export_in_file(manifest: &Manifest, file: &str, symbol: &str) -> Option<ExportLines> {
    let entry = manifest.files.get(file)?;
    let index = entry.exports.iter().position(|export| export == symbol)?;
    entry.export_lines.as_ref()?.get(index).cloned()
}

fn resolve_bare_symbol(
    manifest: &Manifest,
    root: &Path,
    name: &str,
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    let export_locations = concrete_export_locations(manifest, name);
    match export_locations.len() {
        0 => {}
        1 => {
            let location = export_locations.into_iter().next().expect("length checked");
            return Ok((location.file, location.lines));
        }
        _ => return Err(guidance.ambiguous_export(name, &export_locations)),
    }

    if manifest.export_locations.contains_key(name) {
        return resolve_export(manifest, name, guidance);
    }

    let top_level_locations = non_exported_top_level_locations(manifest, root, name);
    match top_level_locations.len() {
        0 => Err(guidance.missing_export(name)),
        1 => {
            let location = top_level_locations
                .into_iter()
                .next()
                .expect("length checked");
            Ok((location.file, location.lines))
        }
        _ => Err(guidance.ambiguous_top_level_function(name, &top_level_locations)),
    }
}

fn concrete_export_locations(manifest: &Manifest, name: &str) -> Vec<SymbolLocation> {
    let Some(locations) = manifest.export_all.get(name) else {
        return Vec::new();
    };

    let mut concrete: Vec<SymbolLocation> = locations
        .iter()
        .filter(|location| !is_reexport_location(manifest, name, &location.file))
        .map(symbol_location_from_export)
        .collect();

    if concrete.is_empty() {
        concrete = locations.iter().map(symbol_location_from_export).collect();
    }

    sort_and_dedup_locations(concrete)
}

fn symbol_location_from_export(location: &ExportLocation) -> SymbolLocation {
    SymbolLocation {
        file: location.file.clone(),
        lines: location.lines.clone(),
    }
}

fn is_reexport_location(manifest: &Manifest, name: &str, file: &str) -> bool {
    if crate::mcp::tools::is_reexport_file(file) {
        return true;
    }

    manifest
        .files
        .get(file)
        .map(|entry| {
            entry
                .named_imports
                .values()
                .any(|names| names.iter().any(|candidate| candidate == name))
        })
        .unwrap_or(false)
}

fn non_exported_top_level_locations(
    manifest: &Manifest,
    root: &Path,
    name: &str,
) -> Vec<SymbolLocation> {
    let mut matches = Vec::new();
    for (file, entry) in &manifest.files {
        let export_names: Vec<&str> = entry.exports.iter().map(|export| export.as_str()).collect();
        let top_level_fns = fmm_core::manifest::private_members::extract_top_level_functions(
            root,
            file,
            &export_names,
        );
        for function in top_level_fns {
            if function.name == name {
                matches.push(SymbolLocation {
                    file: file.clone(),
                    lines: Some(ExportLines {
                        start: function.start,
                        end: function.end,
                    }),
                });
            }
        }
    }
    sort_and_dedup_locations(matches)
}

fn sort_and_dedup_locations(mut locations: Vec<SymbolLocation>) -> Vec<SymbolLocation> {
    locations.sort_by(|a, b| a.file.cmp(&b.file));
    locations.dedup_by(|a, b| a.file == b.file);
    locations
}

fn resolve_dotted_notation(
    manifest: &Manifest,
    root: &Path,
    name: &str,
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    if let Some(loc) = manifest.method_index.get(name) {
        return Ok((loc.file.clone(), loc.lines.clone()));
    }

    let dot = name.rfind('.').expect("name contains dot");
    let class_name = &name[..dot];
    let method_name = &name[dot + 1..];

    let class_files = class_export_files(manifest, class_name);
    if class_files.is_empty() {
        return Err(guidance.unknown_class(name, class_name));
    }

    for class_file in &class_files {
        if let Some((start, end)) = fmm_core::manifest::private_members::find_private_method_range(
            root,
            class_file,
            class_name,
            method_name,
        ) {
            return Ok((
                class_file.clone(),
                Some(fmm_core::manifest::ExportLines { start, end }),
            ));
        }
    }

    let class_file = class_files
        .first()
        .expect("class_files is not empty after guard");
    Err(guidance.missing_method(name, method_name, class_name, class_file))
}

fn resolve_export(
    manifest: &Manifest,
    name: &str,
    guidance: ReadSymbolGuidance,
) -> Result<(String, Option<ExportLines>), String> {
    let location = manifest
        .export_locations
        .get(name)
        .ok_or_else(|| guidance.missing_export(name))?;

    if crate::mcp::tools::is_reexport_file(&location.file)
        && let Some((concrete_file, concrete_lines)) =
            crate::mcp::tools::find_concrete_definition(manifest, name, &location.file)
    {
        Ok((concrete_file, Some(concrete_lines)))
    } else {
        Ok((location.file.clone(), location.lines.clone()))
    }
}

fn class_export_files(manifest: &Manifest, class_name: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(location) = manifest.export_locations.get(class_name) {
        files.push(location.file.clone());
    }
    if let Some(locations) = manifest.export_all.get(class_name) {
        for location in locations {
            if !files.contains(&location.file) {
                files.push(location.file.clone());
            }
        }
    }
    files
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
