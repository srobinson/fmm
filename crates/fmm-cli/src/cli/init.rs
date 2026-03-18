use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use fmm_core::config::Config;

use super::collect_files;
use super::sidecar;

const SKILL_CONTENT: &str = include_str!("../../templates/SKILL.md");

pub fn init(skill: bool, mcp: bool, all: bool, no_generate: bool) -> Result<()> {
    println!(
        "\n{}",
        "Frontmatter Matters — SQLite code intelligence for LLM navigation"
            .cyan()
            .bold()
    );
    println!();

    let specific = skill || mcp;
    let full_setup = !specific || all;

    let install_config = full_setup;
    let install_skill = skill || all;
    let install_mcp = mcp || all;

    if install_config {
        init_config()?;
    }
    if install_skill {
        init_skill()?;
    }
    if install_mcp {
        init_mcp_config()?;
    }

    // Auto-generate index unless --no-generate or partial install
    if full_setup && !no_generate {
        println!();
        let config = Config::load().unwrap_or_default();
        let (files, _) = collect_files(".", &config)?;

        if !files.is_empty() {
            // Detect languages present
            let mut lang_set = std::collections::BTreeSet::new();
            for file in &files {
                if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
                    lang_set.insert(ext.to_string());
                }
            }
            println!(
                "{} {} source files detected ({})",
                "✓".green(),
                files.len(),
                lang_set.into_iter().collect::<Vec<_>>().join(", ")
            );

            sidecar::generate(&[".".to_string()], false, false, false)?;

            // Show DB stats and a sample export
            let root = super::resolve_root(".")?;
            if let Ok(conn) = fmm_store::open_db(&root) {
                let file_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
                    .unwrap_or(0);
                let export_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM exports", [], |r| r.get(0))
                    .unwrap_or(0);
                println!(
                    "\n  {} {} files indexed, {} exports",
                    "✓".green(),
                    file_count,
                    export_count
                );

                if let Ok(manifest) = crate::manifest_ext::load_manifest(&root)
                    && let Some((export_name, _)) = manifest.export_index.iter().next()
                {
                    println!(
                        "\n  {} Try: fmm search --export {}",
                        "next:".cyan(),
                        export_name
                    );
                }
            }
        } else {
            println!(
                "{} No supported source files found — index will be created when you add code",
                "!".yellow()
            );
        }
    }

    println!();
    println!("{}", "Setup complete!".green().bold());
    if install_config {
        println!("  Config:   .fmmrc.toml");
    }
    if install_skill {
        println!("  Skill:    .claude/skills/fmm-navigate/SKILL.md");
    }
    if install_mcp {
        println!("  MCP:      .claude/fmm.local.json");
    }

    if install_config {
        println!(
            "  {} Add '.fmm.db' to your .gitignore — the index is regeneratable",
            "hint:".cyan()
        );
        println!(
            "  {} .fmmrc.toml is optional — delete it to use built-in defaults",
            "hint:".cyan()
        );
    }

    if no_generate || specific {
        println!(
            "\n  {} Run 'fmm generate' to index your codebase",
            "next:".cyan()
        );
    } else {
        println!(
            "\n  {} Your AI assistant now navigates this codebase via the fmm index",
            "✓".green()
        );
    }

    Ok(())
}

const FMMRC_TEMPLATE: &str = r#"# fmm configuration
# Only include fields you want to override — defaults apply for everything else.

# Maximum lines per file. Files exceeding this limit are skipped during indexing.
# Default: 100000
# max_lines = 100_000

# Glob patterns to exclude (in addition to .gitignore and .fmmignore).
# exclude = ["benchmarks/fixtures/**", "vendor/**"]

# Override which file extensions to index (default: 29 languages).
# languages = ["ts", "tsx", "js", "jsx", "py", "rs"]

# Test file detection patterns.
# [test_patterns]
# path_contains = ["/test/", "/tests/", "/spec/", "/e2e/", "/__tests__/"]
# filename_suffixes = [".spec.ts", ".test.ts", ".test.js", "_test.go", "_test.rs"]
"#;

fn init_config() -> Result<()> {
    let toml_path = Path::new(".fmmrc.toml");
    if toml_path.exists() {
        println!("{} .fmmrc.toml already exists (skipping)", "!".yellow());
        return Ok(());
    }
    let json_path = Path::new(".fmmrc.json");
    if json_path.exists() {
        println!(
            "{} .fmmrc.json found — consider migrating to .fmmrc.toml (skipping)",
            "!".yellow()
        );
        return Ok(());
    }

    std::fs::write(toml_path, FMMRC_TEMPLATE).context("Failed to write .fmmrc.toml")?;

    println!("{} Created .fmmrc.toml", "✓".green());
    Ok(())
}

pub fn init_skill() -> Result<()> {
    let skill_dir = Path::new(".claude").join("skills").join("fmm-navigate");
    let skill_path = skill_dir.join("SKILL.md");

    std::fs::create_dir_all(&skill_dir)
        .context("Failed to create .claude/skills/fmm-navigate/ directory")?;

    if skill_path.exists() {
        let existing =
            std::fs::read_to_string(&skill_path).context("Failed to read existing skill file")?;
        if existing == SKILL_CONTENT {
            println!(
                "{} .claude/skills/fmm-navigate/SKILL.md already up to date (skipping)",
                "!".yellow()
            );
            return Ok(());
        }
    }

    std::fs::write(&skill_path, SKILL_CONTENT).context("Failed to write skill file")?;

    println!(
        "{} Installed Claude skill at .claude/skills/fmm-navigate/SKILL.md",
        "✓".green()
    );
    Ok(())
}

pub fn init_mcp_config() -> Result<()> {
    let claude_dir = Path::new(".claude");
    let config_path = claude_dir.join("fmm.local.json");

    // Ensure .claude/ dir exists
    std::fs::create_dir_all(claude_dir).context("Failed to create .claude/ directory")?;

    if config_path.exists() {
        println!(
            "{} .claude/fmm.local.json already exists (skipping)",
            "!".yellow()
        );
        return Ok(());
    }

    let config = serde_json::json!({
        "mcpServers": {
            "fmm": {
                "command": "fmm",
                "args": ["mcp"]
            }
        }
    });

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(&config_path, format!("{}\n", json))
        .context("Failed to write .claude/fmm.local.json")?;

    println!(
        "{} Created .claude/fmm.local.json with MCP server configuration",
        "✓".green()
    );
    Ok(())
}
