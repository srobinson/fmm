use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use crate::config::Config;
use crate::extractor::sidecar_path_for;

use super::collect_files;
use super::resolve_root;
use super::sidecar;

const SKILL_CONTENT: &str = include_str!("../../docs/fmm-navigate.md");

pub fn init(skill: bool, mcp: bool, all: bool, no_generate: bool) -> Result<()> {
    println!(
        "\n{}",
        "Frontmatter Matters — metadata sidecars for LLM code navigation"
            .cyan()
            .bold()
    );
    println!();

    let specific = skill || mcp;
    let full_setup = !specific || all;

    let install_config = full_setup;
    let install_skill = skill || full_setup;
    let install_mcp = mcp || full_setup;

    if install_config {
        init_config()?;
    }
    if install_skill {
        init_skill()?;
    }
    if install_mcp {
        init_mcp_config()?;
    }

    // Auto-generate sidecars unless --no-generate or partial install
    if full_setup && !no_generate {
        println!();
        let config = Config::load().unwrap_or_default();
        let files = collect_files(".", &config)?;

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

            println!("{}", "Generating sidecars...".green().bold());
            sidecar::generate(".", false)?;

            // Show one sample sidecar
            let root = resolve_root(".")?;
            if let Some(sample_file) = files.iter().find(|f| sidecar_path_for(f).exists()) {
                let sidecar = sidecar_path_for(sample_file);
                if let Ok(content) = std::fs::read_to_string(&sidecar) {
                    let rel = sample_file
                        .strip_prefix(&root)
                        .unwrap_or(sample_file)
                        .display();
                    println!(
                        "\n{} {}:",
                        "Sample sidecar for".dimmed(),
                        rel.to_string().white().bold()
                    );
                    for line in content.lines().take(15) {
                        println!("  {}", line.dimmed());
                    }
                    if content.lines().count() > 15 {
                        println!("  {}", "...".dimmed());
                    }
                }

                // Suggest a search using a real export
                let manifest = crate::manifest::Manifest::load_from_sidecars(&root)?;
                if let Some((export_name, _)) = manifest.export_index.iter().next() {
                    println!(
                        "\n  {} Try: fmm search --export {}",
                        "next:".cyan(),
                        export_name
                    );
                }
            }
        } else {
            println!(
                "{} No supported source files found — sidecars will be created when you add code",
                "!".yellow()
            );
        }
    }

    println!();
    println!("{}", "Setup complete!".green().bold());
    if install_config {
        println!("  Config:   .fmmrc.json");
    }
    if install_skill {
        println!("  Skill:    .claude/skills/fmm-navigate.md");
    }
    if install_mcp {
        println!("  MCP:      .mcp.json");
    }

    if no_generate || specific {
        println!(
            "\n  {} Run 'fmm generate' to create sidecars — your AI assistant will navigate via metadata",
            "next:".cyan()
        );
    } else {
        println!(
            "\n  {} Your AI assistant now navigates this codebase via metadata sidecars",
            "✓".green()
        );
    }

    Ok(())
}

fn init_config() -> Result<()> {
    let config_path = Path::new(".fmmrc.json");
    if config_path.exists() {
        println!("{} .fmmrc.json already exists (skipping)", "!".yellow());
        return Ok(());
    }

    let default_config = Config::default();
    let json = serde_json::to_string_pretty(&default_config)?;
    std::fs::write(config_path, format!("{}\n", json)).context("Failed to write .fmmrc.json")?;

    println!(
        "{} Created .fmmrc.json with default configuration",
        "✓".green()
    );
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
    let mcp_path = Path::new(".mcp.json");

    let mcp_config = serde_json::json!({
        "mcpServers": {
            "fmm": {
                "command": "fmm",
                "args": ["mcp"]
            }
        }
    });

    if mcp_path.exists() {
        let existing =
            std::fs::read_to_string(mcp_path).context("Failed to read existing .mcp.json")?;
        if let Ok(mut existing_json) = serde_json::from_str::<serde_json::Value>(&existing) {
            if let Some(servers) = existing_json.get("mcpServers").and_then(|s| s.as_object()) {
                if servers.contains_key("fmm") {
                    println!(
                        "{} .mcp.json already has fmm server configured (skipping)",
                        "!".yellow()
                    );
                    return Ok(());
                }
            }
            if let Some(obj) = existing_json.as_object_mut() {
                let servers = obj
                    .entry("mcpServers")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(servers_obj) = servers.as_object_mut() {
                    servers_obj.insert(
                        "fmm".to_string(),
                        serde_json::json!({
                            "command": "fmm",
                            "args": ["mcp"]
                        }),
                    );
                }
            }
            let json = serde_json::to_string_pretty(&existing_json)?;
            std::fs::write(mcp_path, format!("{}\n", json)).context("Failed to write .mcp.json")?;
            println!("{} Added fmm server to existing .mcp.json", "✓".green());
            return Ok(());
        }
    }

    let json = serde_json::to_string_pretty(&mcp_config)?;
    std::fs::write(mcp_path, format!("{}\n", json)).context("Failed to write .mcp.json")?;

    println!(
        "{} Created .mcp.json with fmm server configuration",
        "✓".green()
    );
    Ok(())
}
