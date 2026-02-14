use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use color_print::cstr;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

use crate::config::Config;

pub mod init;
mod search;
mod sidecar;
mod status;

pub use init::init;
pub use init::init_skill;
pub use search::search;
pub use sidecar::{clean, generate, update, validate};
pub use status::status;

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
  <bold>completions</bold>   Generate shell completions (bash, zsh, fish, powershell, elvish)

<bold><underline>Integration</underline></bold>
  <bold>mcp</bold>           Start MCP server for LLM tool integration

<bold><underline>Analysis</underline></bold>
  <bold>search</bold>        Query sidecars by export, import, dependency, or LOC
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
