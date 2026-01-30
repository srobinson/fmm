use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::extractor::{sidecar_path_for, FileProcessor};

#[derive(Parser)]
#[command(
    name = "fmm",
    about = "Frontmatter Matters - Auto-generate code metadata sidecars for LLM navigation",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate .fmm sidecar files for source files that don't have them
    Generate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be changed
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Update all .fmm sidecar files (regenerate from source)
    Update {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be changed
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Validate that .fmm sidecars are up to date
    Validate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Remove all .fmm sidecar files (and legacy .fmm/ directory)
    Clean {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be removed
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Initialize fmm in this project (config, skill, MCP)
    Init {
        /// Install Claude Code skill only (.claude/skills/fmm-navigate.md)
        #[arg(long)]
        skill: bool,

        /// Install MCP server config only
        #[arg(long)]
        mcp: bool,

        /// Install all integrations (non-interactive)
        #[arg(long)]
        all: bool,
    },

    /// Show current fmm status and configuration
    Status,

    /// Search sidecars for files and exports
    Search {
        /// Find file by export name
        #[arg(short = 'e', long = "export")]
        export: Option<String>,

        /// Find files that import a module
        #[arg(short = 'i', long = "imports")]
        imports: Option<String>,

        /// Filter by line count (e.g., ">500", "<100", "=200")
        #[arg(short = 'l', long = "loc")]
        loc: Option<String>,

        /// Find files that depend on a path
        #[arg(short = 'd', long = "depends-on")]
        depends_on: Option<String>,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Start MCP (Model Context Protocol) server for LLM integration
    Mcp,

    /// Start MCP server for LLM integration (alias for 'mcp')
    Serve,

    /// Compare FMM vs control performance on a GitHub repository
    Compare {
        /// GitHub repository URL (e.g., https://github.com/owner/repo)
        url: String,

        /// Branch to compare (default: main)
        #[arg(short, long)]
        branch: Option<String>,

        /// Path within repo to analyze
        #[arg(long)]
        src_path: Option<String>,

        /// Task set to use (standard, quick, or path to custom JSON)
        #[arg(long, default_value = "standard")]
        tasks: String,

        /// Number of runs per task
        #[arg(long, default_value = "1")]
        runs: u32,

        /// Output directory for results
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format
        #[arg(long, value_enum, default_value = "both")]
        format: OutputFormat,

        /// Maximum budget in USD
        #[arg(long, default_value = "10.0")]
        max_budget: f64,

        /// Skip cache (always re-run tasks)
        #[arg(long)]
        no_cache: bool,

        /// Quick mode (fewer tasks, faster results)
        #[arg(long)]
        quick: bool,

        /// Model to use
        #[arg(long, default_value = "sonnet")]
        model: String,
    },
}

/// Output format for comparison reports
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Markdown,
    Both,
}

/// Resolve the root directory from the target path.
/// If a directory, use it directly. If a file, use its parent.
/// Falls back to CWD if the path doesn't exist.
fn resolve_root(path: &str) -> Result<PathBuf> {
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    if target.is_dir() {
        Ok(target)
    } else if target.is_file() {
        match target.parent() {
            Some(p) => Ok(p.to_path_buf()),
            None => std::env::current_dir().context("Failed to get current directory"),
        }
    } else {
        std::env::current_dir().context("Failed to get current directory")
    }
}

pub fn generate(path: &str, dry_run: bool) -> Result<()> {
    // Safe default: missing/invalid config falls back to sensible defaults (no ignores, standard settings)
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config, &root);
            match processor.generate(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("{} {}: {}", "Error".red(), file.display(), e);
                    None
                }
            }
        })
        .collect();

    for (file, msg) in &results {
        let sidecar = sidecar_path_for(file);
        let display = sidecar.strip_prefix(&root).unwrap_or(&sidecar).display();
        println!("{} {}", "✓".green(), display);
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    if !results.is_empty() {
        println!(
            "\n{} {} sidecar(s) {}",
            "✓".green().bold(),
            results.len(),
            if dry_run {
                "would be written"
            } else {
                "written"
            }
        );
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn update(path: &str, dry_run: bool) -> Result<()> {
    // Safe default: missing/invalid config falls back to sensible defaults (no ignores, standard settings)
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config, &root);
            match processor.update(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("{} {}: {}", "Error".red(), file.display(), e);
                    None
                }
            }
        })
        .collect();

    for (file, msg) in &results {
        let sidecar = sidecar_path_for(file);
        let display = sidecar.strip_prefix(&root).unwrap_or(&sidecar).display();
        println!("{} {}", "✓".green(), display);
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    if !results.is_empty() {
        println!(
            "\n{} {} sidecar(s) {}",
            "✓".green().bold(),
            results.len(),
            if dry_run {
                "would be updated"
            } else {
                "updated"
            }
        );
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn validate(path: &str) -> Result<()> {
    // Safe default: missing/invalid config falls back to sensible defaults (no ignores, standard settings)
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    println!("Validating {} files...", files.len());

    let invalid: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config, &root);
            match processor.validate(file) {
                Ok(true) => None,
                Ok(false) => {
                    let sidecar = sidecar_path_for(file);
                    let reason = if sidecar.exists() {
                        "sidecar out of date"
                    } else {
                        "missing sidecar"
                    };
                    Some((file.to_path_buf(), reason.to_string()))
                }
                Err(e) => Some((file.to_path_buf(), format!("Error: {}", e))),
            }
        })
        .collect();

    if invalid.is_empty() {
        println!("{} All sidecars are up to date!", "✓".green().bold());
        Ok(())
    } else {
        println!(
            "{} {} files need updating:",
            "✗".red().bold(),
            invalid.len()
        );
        for (file, msg) in &invalid {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            println!("  {} {}: {}", "✗".red(), rel.display(), msg.dimmed());
        }
        anyhow::bail!("Sidecar validation failed");
    }
}

pub fn clean(path: &str, dry_run: bool) -> Result<()> {
    // Safe default: missing/invalid config falls back to sensible defaults (no ignores, standard settings)
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    let mut removed = 0u32;

    for file in &files {
        let processor = FileProcessor::new(&config, &root);
        match processor.clean(file) {
            Ok(true) => {
                let sidecar = sidecar_path_for(file);
                let display = sidecar
                    .strip_prefix(&root)
                    .unwrap_or(&sidecar)
                    .display()
                    .to_string();
                if dry_run {
                    println!("  Would remove: {}", display);
                } else {
                    println!("{} Removed {}", "✓".green(), display);
                }
                removed += 1;
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("{} {}: {}", "Error".red(), file.display(), e);
            }
        }
    }

    // Also clean legacy .fmm/ directory
    let legacy_dir = root.join(".fmm");
    if legacy_dir.is_dir() {
        if dry_run {
            println!("  Would remove legacy directory: .fmm/");
        } else {
            std::fs::remove_dir_all(&legacy_dir)?;
            println!("{} Removed legacy .fmm/ directory", "✓".green());
        }
    }

    println!(
        "\n{} {} sidecar(s) {}",
        "✓".green().bold(),
        removed,
        if dry_run {
            "would be removed"
        } else {
            "removed"
        }
    );

    Ok(())
}

pub fn init(skill: bool, mcp: bool, all: bool) -> Result<()> {
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

    println!();
    println!("{}", "Setup complete!".green().bold());
    if install_skill {
        println!("  Skill:    .claude/skills/fmm-navigate.md");
    }
    if install_mcp {
        println!("  MCP:      .mcp.json");
    }
    println!("\nRun `fmm generate` to create sidecar files.");

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

const SKILL_CONTENT: &str = include_str!("../../docs/fmm-navigate.md");

pub fn init_skill() -> Result<()> {
    let skill_dir = Path::new(".claude").join("skills");
    let skill_path = skill_dir.join("fmm-navigate.md");

    std::fs::create_dir_all(&skill_dir).context("Failed to create .claude/skills/ directory")?;

    if skill_path.exists() {
        let existing =
            std::fs::read_to_string(&skill_path).context("Failed to read existing skill file")?;
        if existing == SKILL_CONTENT {
            println!(
                "{} .claude/skills/fmm-navigate.md already up to date (skipping)",
                "!".yellow()
            );
            return Ok(());
        }
    }

    std::fs::write(&skill_path, SKILL_CONTENT).context("Failed to write skill file")?;

    println!(
        "{} Installed Claude skill at .claude/skills/fmm-navigate.md",
        "✓".green()
    );
    Ok(())
}

fn init_mcp_config() -> Result<()> {
    let mcp_path = Path::new(".mcp.json");

    let mcp_config = serde_json::json!({
        "mcpServers": {
            "fmm": {
                "command": "fmm",
                "args": ["serve"]
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
                            "args": ["serve"]
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

pub fn status() -> Result<()> {
    let config_path = Path::new(".fmmrc.json");
    let config_exists = config_path.exists();
    // Safe default: missing/invalid config falls back to sensible defaults (no ignores, standard settings)
    let config = Config::load().unwrap_or_default();

    println!("{}", "fmm Status".cyan().bold());
    println!("{}", "=".repeat(40).dimmed());

    println!("\n{}", "Configuration:".yellow().bold());
    if config_exists {
        println!("  {} .fmmrc.json found", "✓".green());
    } else {
        println!("  {} No .fmmrc.json (using defaults)", "!".yellow());
    }

    println!("\n{}", "Settings:".yellow().bold());
    let format_str = match config.format {
        crate::config::FrontmatterFormat::Yaml => "YAML",
        crate::config::FrontmatterFormat::Json => "JSON",
    };
    println!("  Format:         {}", format_str.white().bold());
    println!(
        "  Include LOC:    {}",
        if config.include_loc {
            "yes".green()
        } else {
            "no".dimmed()
        }
    );
    println!("  Max file size:  {} KB", config.max_file_size);

    println!("\n{}", "Supported Languages:".yellow().bold());
    let mut langs: Vec<_> = config.languages.iter().collect();
    langs.sort();
    println!(
        "  {}",
        langs
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    println!("\n{}", "Workspace:".yellow().bold());
    // Safe default: empty path is harmless for display-only usage
    let cwd = std::env::current_dir().unwrap_or_default();
    println!("  Path: {}", cwd.display());

    match collect_files(".", &config) {
        Ok(files) => {
            let sidecar_count = files
                .iter()
                .filter(|f| sidecar_path_for(f).exists())
                .count();
            println!(
                "  {} source files, {} sidecars",
                files.len().to_string().white().bold(),
                sidecar_count.to_string().white().bold()
            );
        }
        Err(e) => {
            println!("  {} Error scanning: {}", "✗".red(), e);
        }
    }

    println!();
    Ok(())
}

fn collect_files(path: &str, config: &Config) -> Result<Vec<PathBuf>> {
    let path = Path::new(path);

    if path.is_file() {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        return Ok(vec![canonical]);
    }

    let walker = WalkBuilder::new(path)
        .standard_filters(true)
        .add_custom_ignore_filename(".fmmignore")
        .build();

    let files: Vec<PathBuf> = walker
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| {
            if let Some(ext) = entry.path().extension() {
                config.is_supported_language(ext.to_str().unwrap_or(""))
            } else {
                false
            }
        })
        .map(|entry| {
            entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf())
        })
        .collect();

    Ok(files)
}

/// Search result for JSON output
#[derive(serde::Serialize)]
struct SearchResult {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loc: Option<usize>,
}

pub fn search(
    export: Option<String>,
    imports: Option<String>,
    loc: Option<String>,
    depends_on: Option<String>,
    json_output: bool,
) -> Result<()> {
    let root = std::env::current_dir()?;
    let manifest = crate::manifest::Manifest::load_from_sidecars(&root)?;

    let mut results: Vec<SearchResult> = Vec::new();

    // Search by export name (uses reverse index)
    if let Some(ref export_name) = export {
        if let Some(file_path) = manifest.export_index.get(export_name) {
            if let Some(entry) = manifest.files.get(file_path) {
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
            }
        }
    }

    // Search by imports
    if let Some(ref import_name) = imports {
        for (file_path, entry) in &manifest.files {
            if entry
                .imports
                .iter()
                .any(|i| i.contains(import_name.as_str()))
            {
                if results.iter().any(|r| r.file == *file_path) {
                    continue;
                }
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
            }
        }
    }

    // Search by dependencies
    if let Some(ref dep_path) = depends_on {
        for (file_path, entry) in &manifest.files {
            if entry
                .dependencies
                .iter()
                .any(|d| d.contains(dep_path.as_str()))
            {
                if results.iter().any(|r| r.file == *file_path) {
                    continue;
                }
                results.push(SearchResult {
                    file: file_path.clone(),
                    exports: Some(entry.exports.clone()),
                    imports: Some(entry.imports.clone()),
                    dependencies: Some(entry.dependencies.clone()),
                    loc: Some(entry.loc),
                });
            }
        }
    }

    // Filter by LOC
    if let Some(ref loc_expr) = loc {
        let (op, value) = parse_loc_expr(loc_expr)?;

        if export.is_none() && imports.is_none() && depends_on.is_none() {
            for (file_path, entry) in &manifest.files {
                if matches_loc_filter(entry.loc, &op, value) {
                    results.push(SearchResult {
                        file: file_path.clone(),
                        exports: Some(entry.exports.clone()),
                        imports: Some(entry.imports.clone()),
                        dependencies: Some(entry.dependencies.clone()),
                        loc: Some(entry.loc),
                    });
                }
            }
        } else {
            results.retain(|r| r.loc.is_some_and(|l| matches_loc_filter(l, &op, value)));
        }
    }

    // If no filters provided, list all files
    if export.is_none() && imports.is_none() && depends_on.is_none() && loc.is_none() {
        for (file_path, entry) in &manifest.files {
            results.push(SearchResult {
                file: file_path.clone(),
                exports: Some(entry.exports.clone()),
                imports: Some(entry.imports.clone()),
                dependencies: Some(entry.dependencies.clone()),
                loc: Some(entry.loc),
            });
        }
    }

    results.sort_by(|a, b| a.file.cmp(&b.file));

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() {
        println!("{} No matches found", "!".yellow());
    } else {
        println!("{} {} file(s) found:\n", "✓".green(), results.len());
        for result in &results {
            println!("{}", result.file.white().bold());
            if let Some(ref exports) = result.exports {
                if !exports.is_empty() {
                    println!("  {} {}", "exports:".dimmed(), exports.join(", "));
                }
            }
            if let Some(ref imports) = result.imports {
                if !imports.is_empty() {
                    println!("  {} {}", "imports:".dimmed(), imports.join(", "));
                }
            }
            if let Some(loc_val) = result.loc {
                println!("  {} {}", "loc:".dimmed(), loc_val);
            }
            println!();
        }
    }

    Ok(())
}

fn parse_loc_expr(expr: &str) -> Result<(String, usize)> {
    let expr = expr.trim();

    if let Some(rest) = expr.strip_prefix(">=") {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok((">=".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix("<=") {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("<=".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('>') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok((">".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('<') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("<".to_string(), value))
    } else if let Some(rest) = expr.strip_prefix('=') {
        let value: usize = rest.trim().parse().context("Invalid LOC value")?;
        Ok(("=".to_string(), value))
    } else {
        let value: usize = expr
            .parse()
            .context("Invalid LOC expression. Use: >500, <100, =200, >=50, <=1000")?;
        Ok(("=".to_string(), value))
    }
}

fn matches_loc_filter(loc: usize, op: &str, value: usize) -> bool {
    match op {
        ">" => loc > value,
        "<" => loc < value,
        ">=" => loc >= value,
        "<=" => loc <= value,
        "=" => loc == value,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_root_with_absolute_directory() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_root(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap());
        assert!(result.is_absolute());
    }

    #[test]
    fn resolve_root_with_relative_directory() {
        let result = resolve_root(".").unwrap();
        let expected = std::env::current_dir().unwrap().canonicalize().unwrap();
        assert_eq!(result, expected);
        assert!(result.is_absolute());
    }

    #[test]
    fn resolve_root_with_file_returns_parent() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("example.ts");
        std::fs::write(&file_path, "export const x = 1;").unwrap();

        let result = resolve_root(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_root_nonexistent_path_falls_back_to_cwd() {
        let result = resolve_root("/surely/this/does/not/exist/anywhere").unwrap();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(result, cwd);
    }

    #[test]
    fn collect_files_returns_canonical_paths() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("app.ts"), "export const a = 1;").unwrap();
        std::fs::write(src.join("util.ts"), "export const b = 2;").unwrap();

        let config = Config::default();
        let files = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();

        assert!(!files.is_empty());
        for file in &files {
            assert!(file.is_absolute(), "path should be absolute: {:?}", file);
        }
    }

    #[test]
    fn collect_files_single_file_is_canonical() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("index.ts");
        std::fs::write(&file_path, "export function main() {}").unwrap();

        let config = Config::default();
        let files = collect_files(file_path.to_str().unwrap(), &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].is_absolute());
        assert_eq!(files[0], file_path.canonicalize().unwrap());
    }
}
