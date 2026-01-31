use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use color_print::cstr;
use colored::Colorize;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::extractor::{sidecar_path_for, FileProcessor};

// -- Help text constants (keeps the derive attrs readable) --

const LONG_ABOUT: &str = "\
Frontmatter Matters (fmm) generates .fmm sidecar files alongside your source code. \
Each sidecar is a small YAML file listing the exports, imports, dependencies, and \
line count of its companion source file.

LLM agents use these sidecars to navigate codebases without reading every source \
file — reducing token usage by 80-90% while maintaining full structural awareness.

Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C++, C#, Ruby";

const AFTER_LONG_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm init</bold>
    Set up config, Claude skill, and MCP server in one step

  <dim>$</dim> <bold>fmm generate</bold>
    Create .fmm sidecars for all source files in the current directory

  <dim>$</dim> <bold>fmm generate src/</bold>
    Generate sidecars for a specific directory only

  <dim>$</dim> <bold>fmm search --export createStore</bold>
    Find which file defines a symbol (O(1) lookup via reverse index)

  <dim>$</dim> <bold>fmm search --loc ">500"</bold>
    Find large files (over 500 lines)

  <dim>$</dim> <bold>fmm validate</bold>
    Check all sidecars are current — great for CI pipelines

<bold><underline>Learn more</underline></bold>

  https://github.com/mdcontext/fmm"#
);

const BEFORE_LONG_HELP: &str = cstr!(
    r#"<bold><underline>Core Commands</underline></bold>
  <bold>generate</bold>      Create .fmm sidecar files for source files
  <bold>update</bold>        Regenerate all .fmm sidecars from source
  <bold>validate</bold>      Check sidecars are up to date (CI-friendly)
  <bold>clean</bold>         Remove all .fmm sidecar files

<bold><underline>Setup</underline></bold>
  <bold>init</bold>          Initialize fmm in this project (config, skill, MCP)
  <bold>status</bold>        Show current fmm status and configuration
  <bold>completions</bold>   Generate shell completions (bash, zsh, fish, powershell)

<bold><underline>Integration</underline></bold>
  <bold>mcp</bold>           Start MCP server for LLM tool integration
  <bold>gh</bold>            GitHub integrations (issue fixing, PR creation)

<bold><underline>Analysis</underline></bold>
  <bold>search</bold>        Query sidecars by export, import, dependency, or LOC
  <bold>compare</bold>       Benchmark FMM vs control on a GitHub repository
"#
);

#[derive(Parser)]
#[command(
    name = "fmm",
    about = "Auto-generate code metadata sidecars for LLM navigation",
    long_about = LONG_ABOUT,
    before_long_help = BEFORE_LONG_HELP,
    after_long_help = AFTER_LONG_HELP,
    version,
)]
pub struct Cli {
    /// Print CLI reference as Markdown and exit
    #[arg(long, hide = true)]
    pub markdown_help: bool,

    /// Generate man pages to the specified directory and exit
    #[arg(long, hide = true)]
    pub generate_man_pages: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create .fmm sidecar files for source files
    #[command(
        long_about = "Create .fmm sidecar files for source files that don't already have them.\n\n\
            Each sidecar captures the file's exports, imports, dependencies, and line count \
            in a compact YAML format. Existing sidecars are left untouched — use 'update' to \
            refresh them.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm generate</bold>
    Generate sidecars for all supported files in the current directory

  <dim>$</dim> <bold>fmm generate src/</bold>
    Generate sidecars for a specific directory

  <dim>$</dim> <bold>fmm generate -n</bold>
    Dry run — show what would be created without writing files"#),
    )]
    Generate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Show what would be created without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Regenerate all .fmm sidecars from source
    #[command(
        long_about = "Regenerate all .fmm sidecar files from their source files.\n\n\
            Unlike 'generate' which skips existing sidecars, 'update' overwrites every \
            sidecar with fresh metadata. Use after refactoring or when sidecars may be stale.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm update</bold>
    Refresh all sidecars in the current directory

  <dim>$</dim> <bold>fmm update src/ -n</bold>
    Preview which sidecars would change"#),
    )]
    Update {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Show what would be changed without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Check sidecars are up to date (CI-friendly)
    #[command(
        long_about = "Validate that all .fmm sidecars match their source files.\n\n\
            Returns exit code 0 if all sidecars are current, or 1 if any are stale or \
            missing. Designed for CI pipelines — add to your pre-commit hooks or GitHub Actions.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm validate</bold>
    Check all sidecars in the current directory

  <dim>$</dim> <bold>fmm validate src/</bold>
    Check a specific directory"#),
    )]
    Validate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Remove all .fmm sidecar files
    #[command(
        long_about = "Remove all .fmm sidecar files and the legacy .fmm/ directory.\n\n\
            Use this to cleanly uninstall fmm from a project or to start fresh.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm clean</bold>
    Remove all sidecars in the current directory

  <dim>$</dim> <bold>fmm clean -n</bold>
    Preview what would be removed"#),
    )]
    Clean {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Show what would be removed without deleting files
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Initialize fmm in this project (config, skill, MCP)
    #[command(
        long_about = "Set up fmm in the current project.\n\n\
            Creates .fmmrc.json config, installs the Claude Code skill for sidecar-aware \
            navigation, and configures the MCP server in .mcp.json. Run with no flags for \
            the full setup, or use --skill/--mcp to install individual components.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm init</bold>
    Full setup — config, skill, and MCP server

  <dim>$</dim> <bold>fmm init --skill</bold>
    Install only the Claude Code navigation skill

  <dim>$</dim> <bold>fmm init --mcp</bold>
    Install only the MCP server configuration"#),
    )]
    Init {
        /// Install Claude Code skill only (.claude/skills/fmm-navigate.md)
        #[arg(long)]
        skill: bool,

        /// Install MCP server config only (.mcp.json)
        #[arg(long)]
        mcp: bool,

        /// Install all integrations (non-interactive)
        #[arg(long)]
        all: bool,

        /// Skip auto-generating sidecars (config files only)
        #[arg(long)]
        no_generate: bool,
    },

    /// Show current fmm status and configuration
    #[command(
        long_about = "Display the current fmm configuration, supported languages, and \
            workspace statistics including source file and sidecar counts."
    )]
    Status,

    /// Query sidecars by export, import, dependency, or LOC
    #[command(
        long_about = "Search sidecar metadata to find files by export name, import path, \
            dependency, or line count.\n\n\
            Export lookups use a reverse index for O(1) performance. Filters can be combined. \
            With no filters, lists all indexed files.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm search --export createStore</bold>
    Find which file defines 'createStore'

  <dim>$</dim> <bold>fmm search --imports react</bold>
    Find all files that import from 'react'

  <dim>$</dim> <bold>fmm search --loc ">500"</bold>
    Find files over 500 lines

  <dim>$</dim> <bold>fmm search --depends-on src/utils.ts --json</bold>
    Find dependents of a file, output as JSON"#),
    )]
    Search {
        /// Find file by export name (O(1) reverse-index lookup)
        #[arg(short = 'e', long = "export")]
        export: Option<String>,

        /// Find files that import a module
        #[arg(short = 'i', long = "imports")]
        imports: Option<String>,

        /// Filter by line count (e.g., ">500", "<100", "=200")
        #[arg(
            short = 'l',
            long = "loc",
            long_help = "Filter files by line count.\n\n\
                Supports comparison operators: >500, <100, >=50, <=1000, =200.\n\
                A bare number is treated as exact match (=)."
        )]
        loc: Option<String>,

        /// Find files that depend on a path
        #[arg(short = 'd', long = "depends-on")]
        depends_on: Option<String>,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Start MCP server for LLM tool integration
    #[command(
        long_about = "Start the Model Context Protocol (MCP) server over stdio.\n\n\
            The MCP server exposes fmm's search and metadata capabilities as tools that \
            LLM agents (Claude, GPT, etc.) can call directly. Add to .mcp.json with \
            'fmm init --mcp'."
    )]
    Mcp,

    /// Alias for 'mcp'
    #[command(hide = true)]
    Serve,

    /// Generate shell completions for bash, zsh, fish, or powershell
    #[command(
        long_about = "Generate shell completion scripts for fmm.\n\n\
            Outputs a completion script for the specified shell to stdout. \
            Redirect to the appropriate file for your shell to enable tab completion.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm completions bash</bold> > ~/.local/share/bash-completion/completions/fmm
  <dim>$</dim> <bold>fmm completions zsh</bold> > ~/.zfunc/_fmm
  <dim>$</dim> <bold>fmm completions fish</bold> > ~/.config/fish/completions/fmm.fish
  <dim>$</dim> <bold>fmm completions powershell</bold> > _fmm.ps1"#),
    )]
    Completions {
        /// Target shell
        shell: Shell,
    },

    /// GitHub integrations (issue fixing, PR creation)
    #[command(
        long_about = "GitHub workflow integrations powered by fmm sidecar metadata.\n\n\
            Currently supports automated issue fixing: clone a repo, generate sidecars, \
            extract code references from the issue, and invoke Claude with focused context \
            to create a PR."
    )]
    Gh {
        #[command(subcommand)]
        subcommand: GhSubcommand,
    },

    /// Benchmark FMM vs control on a GitHub repository
    #[command(
        long_about = "Run controlled comparisons of FMM-assisted vs unassisted Claude \
            performance on a GitHub repository.\n\n\
            Clones the repo, generates sidecars, runs a set of coding tasks with and \
            without FMM, and produces a report comparing token usage, cost, and quality.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm compare https://github.com/owner/repo</bold>
    Run standard benchmark suite

  <dim>$</dim> <bold>fmm compare https://github.com/owner/repo --quick</bold>
    Quick mode with fewer tasks

  <dim>$</dim> <bold>fmm compare https://github.com/owner/repo --format json -o results/</bold>
    JSON output to a specific directory"#),
    )]
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

/// GitHub subcommands
#[derive(Subcommand)]
pub enum GhSubcommand {
    /// Fix a GitHub issue: clone, generate sidecars, invoke Claude, create PR
    #[command(
        long_about = "Automated GitHub issue fixing powered by fmm.\n\n\
            Pipeline: parse issue URL → fetch issue details → clone repo → generate \
            sidecars → extract code references → resolve against sidecar index → build \
            focused prompt → create branch → invoke Claude → commit → push → create PR.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm gh issue https://github.com/owner/repo/issues/42</bold>
    Fix an issue and create a PR

  <dim>$</dim> <bold>fmm gh issue https://github.com/owner/repo/issues/42 -n</bold>
    Dry run — show extracted refs and assembled prompt

  <dim>$</dim> <bold>fmm gh issue https://github.com/owner/repo/issues/42 --no-pr</bold>
    Fix and commit but skip PR creation"#),
    )]
    Issue {
        /// GitHub issue URL (e.g., https://github.com/owner/repo/issues/123)
        url: String,

        /// Claude model to use
        #[arg(long, default_value = "sonnet")]
        model: String,

        /// Maximum turns for Claude
        #[arg(long, default_value = "30")]
        max_turns: u32,

        /// Maximum budget in USD
        #[arg(long, default_value = "5.0")]
        max_budget: f64,

        /// Show plan without executing (extract refs + assembled prompt)
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Git branch prefix
        #[arg(long, default_value = "fmm")]
        branch_prefix: String,

        /// Commit and push only, skip PR creation
        #[arg(long)]
        no_pr: bool,

        /// Override workspace directory
        #[arg(long)]
        workspace: Option<String>,
    },
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
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        println!(
            "\n  {} Supported languages: {}",
            "hint:".cyan(),
            config
                .languages
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config, &root);
            match processor.generate(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!(
                        "{} {}: {}\n  {} Check file permissions and encoding",
                        "error:".red().bold(),
                        file.display(),
                        e,
                        "hint:".cyan()
                    );
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
        if !dry_run {
            println!(
                "\n  {} Run 'fmm validate' to verify, or 'fmm search --export <name>' to find symbols",
                "next:".cyan()
            );
        }
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn update(path: &str, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        println!(
            "\n  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config, &root);
            match processor.update(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!(
                        "{} {}: {}\n  {} Check file permissions and encoding",
                        "error:".red().bold(),
                        file.display(),
                        e,
                        "hint:".cyan()
                    );
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
        if !dry_run {
            println!(
                "\n  {} Run 'fmm validate' to verify sidecars are consistent",
                "next:".cyan()
            );
        }
    } else {
        println!("{} All sidecars up to date", "✓".green());
    }

    Ok(())
}

pub fn validate(path: &str) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    if files.is_empty() {
        println!("{} No supported source files found", "!".yellow());
        println!(
            "\n  {} Did you mean to run from your project root?",
            "hint:".cyan()
        );
        return Ok(());
    }

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
            "{} {} file(s) need updating:",
            "✗".red().bold(),
            invalid.len()
        );
        for (file, msg) in &invalid {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            println!("  {} {}: {}", "✗".red(), rel.display(), msg.dimmed());
        }
        println!(
            "\n  {} Run 'fmm update' to regenerate stale sidecars, or 'fmm generate' for missing ones",
            "fix:".cyan()
        );
        anyhow::bail!("Sidecar validation failed");
    }
}

pub fn clean(path: &str, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = resolve_root(path)?;

    let mut removed = 0u32;

    for file in &files {
        let sidecar = sidecar_path_for(file);
        if !sidecar.exists() {
            continue;
        }
        let display = sidecar
            .strip_prefix(&root)
            .unwrap_or(&sidecar)
            .display()
            .to_string();
        if dry_run {
            println!("  Would remove: {}", display);
            removed += 1;
        } else {
            let processor = FileProcessor::new(&config, &root);
            match processor.clean(file) {
                Ok(true) => {
                    println!("{} Removed {}", "✓".green(), display);
                    removed += 1;
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!(
                        "{} {}: {}\n  {} Check file permissions",
                        "error:".red().bold(),
                        file.display(),
                        e,
                        "hint:".cyan()
                    );
                }
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
            generate(".", false)?;

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

    if manifest.files.is_empty() {
        println!(
            "{} No .fmm sidecars found in the current directory",
            "!".yellow()
        );
        println!(
            "\n  {} fmm search queries sidecar metadata. Run 'fmm generate' first to create them",
            "hint:".cyan()
        );
        return Ok(());
    }

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
        if export.is_some() {
            println!(
                "\n  {} Export names are case-sensitive. Try 'fmm search' with no filters to list all indexed files",
                "hint:".cyan()
            );
        }
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
