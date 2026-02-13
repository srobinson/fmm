use anyhow::{Context, Result};
use colored::Colorize;
use ignore::WalkBuilder;

use super::init::init_mcp_config;

pub fn run(query: &str, model: &str, max_turns: u32, max_budget: f64) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mcp_path = cwd.join(".mcp.json");
    let mcp_exists = mcp_path.exists();

    let sidecars: usize = WalkBuilder::new(&cwd)
        .hidden(false)
        .git_global(false)
        .git_ignore(false)
        .git_exclude(false)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "fmm"))
        .count();

    if sidecars == 0 {
        println!(
            "{} No .fmm sidecars found in the current directory",
            "!".yellow()
        );
        println!(
            "\n  {} Run 'fmm generate' first to create sidecars",
            "hint:".cyan()
        );
        return Ok(());
    }

    if !mcp_exists {
        println!("{} No .mcp.json found, creating one...", "!".yellow());
        init_mcp_config()?;
    }

    println!("{} Running: {}", "→".cyan(), query.white().italic());
    println!(
        "  {} sidecars, model: {}, max_turns: {}, budget: ${:.2}",
        sidecars, model, max_turns, max_budget
    );
    println!();

    let status = std::process::Command::new("claude")
        .arg("-p")
        .arg(query)
        .arg("--model")
        .arg(model)
        .arg("--max-turns")
        .arg(max_turns.to_string())
        .arg("--max-budget-usd")
        .arg(max_budget.to_string())
        .arg("--mcp-config")
        .arg(".mcp.json")
        .arg("--allowedTools")
        .arg("Read,Glob,Grep,LS,mcp__fmm__fmm_lookup_export,mcp__fmm__fmm_list_exports,mcp__fmm__fmm_file_info,mcp__fmm__fmm_dependency_graph,mcp__fmm__fmm_search,mcp__fmm__fmm_read_symbol,mcp__fmm__fmm_file_outline")
        .current_dir(&cwd)
        .status()
        .context("Failed to run Claude. Is 'claude' CLI installed?")?;

    if !status.success() {
        anyhow::bail!("Claude exited with non-zero status");
    }

    Ok(())
}
