use anyhow::{Context, Result};
use fmm_core::manifest::ExportLines;

use super::{load_manifest, warn_no_sidecars};
use crate::read_symbol::{ReadMethodHint, ReadSymbolContent, read_symbol_result};

#[derive(serde::Serialize)]
struct ReadSymbolJson {
    kind: &'static str,
    symbol: String,
    file: String,
    lines: [usize; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    methods: Vec<ReadMethodJson>,
}

#[derive(serde::Serialize)]
struct ReadMethodJson {
    name: String,
    lines: [usize; 2],
}

impl From<&ReadMethodHint> for ReadMethodJson {
    fn from(method: &ReadMethodHint) -> Self {
        Self {
            name: method.name.clone(),
            lines: export_lines_array(&method.lines),
        }
    }
}

pub fn read_symbol(
    name: &str,
    no_truncate: bool,
    line_numbers: bool,
    json_output: bool,
) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    let mut result = read_symbol_result(&manifest, &root, name, !no_truncate)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("Failed to read symbol '{name}'"))?;

    truncate_cli_source(&mut result.content, no_truncate);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&to_json(&result))?);
    } else {
        println!("{}", result.format_text(line_numbers));
    }

    Ok(())
}

fn truncate_cli_source(content: &mut ReadSymbolContent, no_truncate: bool) {
    let ReadSymbolContent::Source(source) = content else {
        return;
    };

    const TRUNCATE_CAP: usize = 10_240;
    if !no_truncate && source.len() > TRUNCATE_CAP {
        source.truncate(TRUNCATE_CAP);
        if let Some(pos) = source.rfind('\n') {
            source.truncate(pos);
        }
        source.push_str("\n... (truncated, use --no-truncate for full source)");
    }
}

fn to_json(result: &crate::read_symbol::ReadSymbolResult) -> ReadSymbolJson {
    match &result.content {
        ReadSymbolContent::Source(source) => ReadSymbolJson {
            kind: "source",
            symbol: result.symbol.clone(),
            file: result.file.clone(),
            lines: export_lines_array(&result.lines),
            source: Some(source.clone()),
            methods: Vec::new(),
        },
        ReadSymbolContent::ClassRedirect { methods } => ReadSymbolJson {
            kind: "class_redirect",
            symbol: result.symbol.clone(),
            file: result.file.clone(),
            lines: export_lines_array(&result.lines),
            source: None,
            methods: methods.iter().map(ReadMethodJson::from).collect(),
        },
    }
}

fn export_lines_array(lines: &ExportLines) -> [usize; 2] {
    [lines.start, lines.end]
}
