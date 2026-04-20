use anyhow::{Context, Result};
use colored::Colorize;

use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;

pub fn glossary(
    pattern: Option<String>,
    mode: &str,
    limit: Option<usize>,
    precision: &str,
    no_truncate: bool,
    json_output: bool,
) -> Result<()> {
    let pattern = pattern.as_deref().unwrap_or("").trim().to_string();
    if pattern.is_empty() {
        anyhow::bail!(
            "pattern is required; provide a symbol name or substring (e.g. 'run_dispatch', 'config')"
        );
    }

    let root = std::env::current_dir().context("Failed to get current directory")?;
    let manifest = SqliteStore::open(&root)?.load_manifest()?;

    if manifest.files.is_empty() {
        eprintln!(
            "{} No index found. Run {} first.",
            "warning:".yellow(),
            "fmm generate".bold()
        );
        return Ok(());
    }

    let mode = crate::glossary::parse_mode(mode);
    let precision =
        crate::glossary::GlossaryPrecision::parse(precision).map_err(anyhow::Error::msg)?;
    let result = crate::glossary::compute_glossary(
        &manifest,
        &root,
        crate::glossary::GlossaryQuery {
            pattern: &pattern,
            mode,
            limit,
            precision,
        },
    )
    .map_err(anyhow::Error::msg)?;

    if json_output {
        let json = serde_json::to_string_pretty(&crate::glossary::json_entries(&result.entries))?;
        println!("{}", json);
        return Ok(());
    }

    let text = cap_cli_glossary_response(crate::glossary::format_text(&result), no_truncate);
    println!("{}", text);

    Ok(())
}

fn cap_cli_glossary_response(text: String, no_truncate: bool) -> String {
    if no_truncate || text.len() <= crate::mcp::MAX_RESPONSE_BYTES {
        return text;
    }

    let safe_limit = text.floor_char_boundary(crate::mcp::MAX_RESPONSE_BYTES);
    let truncated = &text[..safe_limit];
    let cut_point = truncated.rfind('\n').unwrap_or(safe_limit);
    let mut result = text[..cut_point].to_string();
    let total_lines = text.lines().count();
    let shown_lines = result.lines().count();
    result.push_str(&format!(
        "\n\n[Truncated; showing {}/{} lines. Use --no-truncate to get the full glossary.]",
        shown_lines, total_lines
    ));
    result
}
