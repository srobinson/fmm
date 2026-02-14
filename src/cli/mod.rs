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
Frontmatter Matters — 80-90% fewer file reads for LLM agents";

// Short help (-h): commands + hint to use --help
const SHORT_HELP: &str = cstr!(
    r#"<bold><underline>Commands</underline></bold>
  <bold>init</bold>          Set up config, Claude skill, and MCP server
  <bold>generate</bold>      Create .fmm sidecars (exports, imports, deps, LOC)
  <bold>update</bold>        Regenerate all sidecars from source
  <bold>validate</bold>      Check sidecars are current (CI-friendly, exit 1 if stale)
  <bold>search</bold>        Query the index (O(1) export lookup, dependency graphs)
  <bold>mcp</bold>           Start MCP server (7 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Remove all .fmm sidecars

Use <bold>--help</bold> for MCP tools, workflows, and examples.
https://github.com/srobinson/fmm"#
);

// Full help (--help): commands + MCP tools + workflows + languages
const LONG_HELP: &str = cstr!(
    r#"<bold><underline>Commands</underline></bold>
  <bold>init</bold>          Set up config, Claude skill, and MCP server
  <bold>generate</bold>      Create .fmm sidecars (exports, imports, deps, LOC)
  <bold>update</bold>        Regenerate all sidecars from source
  <bold>validate</bold>      Check sidecars are current (CI-friendly, exit 1 if stale)
  <bold>search</bold>        Query the index (O(1) export lookup, dependency graphs)
  <bold>mcp</bold>           Start MCP server (7 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Remove all .fmm sidecars

<bold><underline>MCP Tools</underline></bold> <dim>(via</dim> <bold>fmm mcp</bold><dim>)</dim>
  <bold>fmm_lookup_export</bold>    Find which file defines a symbol — O(1)
  <bold>fmm_read_symbol</bold>      Extract exact source by symbol name (line ranges)
  <bold>fmm_dependency_graph</bold>  Upstream deps + downstream dependents
  <bold>fmm_file_outline</bold>     Table of contents with line ranges
  <bold>fmm_list_exports</bold>     Search exports by pattern (fuzzy)
  <bold>fmm_file_info</bold>        Structural profile without reading source
  <bold>fmm_search</bold>           Multi-criteria AND queries

<bold><underline>Workflows</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                              <dim># One-command setup</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>          <dim># CI pipeline</dim>
  <dim>$</dim> <bold>fmm search --export createStore</bold>       <dim># O(1) symbol lookup</dim>
  <dim>$</dim> <bold>fmm search --depends-on src/auth.ts</bold>   <dim># Impact analysis</dim>
  <dim>$</dim> <bold>fmm search --loc ">500"</bold>              <dim># Find large files</dim>
  <dim>$</dim> <bold>fmm search --imports react --json</bold>     <dim># Structured output</dim>

<bold><underline>Languages</underline></bold>
  TypeScript · JavaScript · Python · Rust · Go · Java · C++ · C# · Ruby

88-97% token reduction measured on real codebases.
https://github.com/srobinson/fmm"#
);

// Custom help template — our before_help already lists commands,
// so we skip the auto-generated {subcommands} section
const HELP_TEMPLATE: &str = "{about-with-newline}\n{before-help}\n";

#[derive(Parser)]
#[command(
    name = "fmm",
    about = LONG_ABOUT,
    long_about = LONG_ABOUT,
    before_help = SHORT_HELP,
    before_long_help = LONG_HELP,
    help_template = HELP_TEMPLATE,
    version,
    disable_help_subcommand = true,
    subcommand_required = false,
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
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm generate</bold>             <dim># All files in current directory</dim>
  <dim>$</dim> <bold>fmm generate src/</bold>         <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>           <dim># Dry run — preview without writing</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm generate</bold>                       <dim># All files in current directory</dim>
  <dim>$</dim> <bold>fmm generate src/</bold>                   <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate src/auth.ts</bold>            <dim># Single file</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>                     <dim># Dry run — preview without writing</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>        <dim># Generate then verify</dim>

<bold><underline>Notes</underline></bold>
  Skips files that already have .fmm sidecars — use 'update' to refresh.
  Respects .gitignore and .fmmignore for file exclusion.
  Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C++, C#, Ruby."#),
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
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm update</bold>               <dim># Refresh all sidecars</dim>
  <dim>$</dim> <bold>fmm update src/</bold>           <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm update -n</bold>             <dim># Preview what would change</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm update</bold>                         <dim># Refresh all sidecars</dim>
  <dim>$</dim> <bold>fmm update src/</bold>                     <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm update src/auth.ts</bold>              <dim># Single file</dim>
  <dim>$</dim> <bold>fmm update -n</bold>                       <dim># Preview what would change</dim>

<bold><underline>Notes</underline></bold>
  Unlike 'generate', this overwrites existing sidecars with fresh metadata.
  Use after refactoring, renaming exports, or changing imports.
  Skips unchanged files for speed — only rewrites stale sidecars."#),
    )]
    Update {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Show what would be changed without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Check sidecars are up to date (CI-friendly, exit 1 if stale)
    #[command(
        long_about = "Validate that all .fmm sidecars match their source files.\n\n\
            Returns exit code 0 if all sidecars are current, or 1 if any are stale or \
            missing. Designed for CI pipelines — add to your pre-commit hooks or GitHub Actions.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>             <dim># Check all sidecars</dim>
  <dim>$</dim> <bold>fmm validate src/</bold>         <dim># Check specific directory</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>                       <dim># Check all sidecars</dim>
  <dim>$</dim> <bold>fmm validate src/</bold>                   <dim># Check specific directory</dim>

  <dim># CI pipeline:</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>        <dim># Generate then verify</dim>

  <dim># GitHub Actions step:</dim>
  <dim>- run: npx frontmatter-matters validate</dim>

  <dim># Pre-commit hook (.husky/pre-commit):</dim>
  <dim>fmm validate || (echo "Stale sidecars — run 'fmm update'" && exit 1)</dim>

<bold><underline>Notes</underline></bold>
  Exit code 0: all sidecars are current.
  Exit code 1: stale or missing sidecars found.
  Run 'fmm update' to fix stale sidecars, 'fmm generate' for missing ones."#),
    )]
    Validate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Remove all .fmm sidecar files
    #[command(
        long_about = "Remove all .fmm sidecar files from the project.\n\n\
            Use this to cleanly uninstall fmm from a project or to start fresh.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm clean</bold>               <dim># Remove all sidecars</dim>
  <dim>$</dim> <bold>fmm clean -n</bold>             <dim># Preview what would be removed</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm clean</bold>                          <dim># Remove all sidecars</dim>
  <dim>$</dim> <bold>fmm clean src/</bold>                      <dim># Remove from specific directory</dim>
  <dim>$</dim> <bold>fmm clean -n</bold>                        <dim># Preview what would be removed</dim>

<bold><underline>Notes</underline></bold>
  Removes .fmm sidecar files only — source files are never touched.
  Safe to re-run: 'fmm generate' recreates everything from source."#),
    )]
    Clean {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Show what would be removed without deleting files
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Set up config, Claude skill, and MCP server
    #[command(
        long_about = "Set up fmm in the current project.\n\n\
            Creates .fmmrc.json config, installs the Claude Code skill for sidecar-aware \
            navigation, and configures the MCP server in .mcp.json. Run with no flags for \
            the full interactive setup, or use flags to install individual components.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                 <dim># Full interactive setup</dim>
  <dim>$</dim> <bold>fmm init --all</bold>            <dim># Non-interactive — install everything</dim>
  <dim>$</dim> <bold>fmm init --mcp</bold>            <dim># MCP server config only</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                           <dim># Full interactive setup</dim>
  <dim>$</dim> <bold>fmm init --all</bold>                      <dim># Non-interactive — install everything</dim>
  <dim>$</dim> <bold>fmm init --skill</bold>                    <dim># Claude Code navigation skill only</dim>
  <dim>$</dim> <bold>fmm init --mcp</bold>                      <dim># MCP server config only</dim>
  <dim>$</dim> <bold>fmm init --all --no-generate</bold>        <dim># Config files only, skip sidecar gen</dim>

<bold><underline>What gets created</underline></bold>
  <bold>.fmmrc.json</bold>                        Project configuration
  <bold>.claude/skills/fmm-navigate/SKILL.md</bold>  Claude Code navigation skill
  <bold>.mcp.json</bold>                          MCP server configuration

<bold><underline>Notes</underline></bold>
  Safe to re-run — existing files are not overwritten.
  The Claude skill teaches Claude to read sidecars before source files.
  The MCP config enables 7 tools for O(1) symbol lookup and navigation."#),
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

    /// Show config, supported languages, and workspace stats
    #[command(
        long_about = "Display the current fmm configuration, supported languages, and \
            workspace statistics including source file and sidecar counts.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm status</bold>                         <dim># Show config and stats</dim>

<bold><underline>Notes</underline></bold>
  Shows: config file location, supported languages, file/sidecar counts.
  Useful for verifying fmm is set up correctly in a project."#),
    )]
    Status,

    /// Query the index — O(1) export lookup, dependency graphs, LOC filters
    #[command(
        long_about = "Search sidecar metadata to find files by export name, import path, \
            dependency, or line count.\n\n\
            Export lookups use a reverse index for O(1) performance. Filters can be combined \
            with AND logic. With no filters, lists all indexed files.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm search -e createStore</bold>  <dim># O(1) symbol lookup</dim>
  <dim>$</dim> <bold>fmm search -i react</bold>         <dim># Files importing react</dim>
  <dim>$</dim> <bold>fmm search -l ">500"</bold>       <dim># Large files</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim># Symbol lookup (O(1) via reverse index):</dim>
  <dim>$</dim> <bold>fmm search --export createStore</bold>    <dim># Find where createStore is defined</dim>
  <dim>$</dim> <bold>fmm search --export "App"</bold>           <dim># Find the App component</dim>

  <dim># Import analysis:</dim>
  <dim>$</dim> <bold>fmm search --imports react</bold>          <dim># All files importing react</dim>
  <dim>$</dim> <bold>fmm search --imports crypto</bold>         <dim># Find crypto usage across codebase</dim>

  <dim># Dependency graph (impact analysis):</dim>
  <dim>$</dim> <bold>fmm search --depends-on src/auth.ts</bold> <dim># What breaks if auth changes?</dim>
  <dim>$</dim> <bold>fmm search --depends-on src/db.ts</bold>   <dim># Downstream dependents of db</dim>

  <dim># Line count filters:</dim>
  <dim>$</dim> <bold>fmm search --loc ">>500"</bold>            <dim># Large files (over 500 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc "<<50"</bold>             <dim># Small files (under 50 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc ">>100" --loc "<<300"</bold>    <dim># Range query (100-300 lines)</dim>

  <dim># Combined filters (AND logic):</dim>
  <dim>$</dim> <bold>fmm search --imports react --loc ">>200"</bold>  <dim># Large React files</dim>

  <dim># Structured output:</dim>
  <dim>$</dim> <bold>fmm search --export App --json</bold>      <dim># JSON for scripting/piping</dim>
  <dim>$</dim> <bold>fmm search --json</bold>                   <dim># All indexed files as JSON</dim>

<bold><underline>Notes</underline></bold>
  Export lookup is O(1) — uses a pre-built reverse index, not file scanning.
  With no filters, lists every indexed file with its metadata.
  Combine filters to narrow results: all filters use AND logic.
  Use --json for machine-readable output (piping, scripts, CI)."#),
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

    /// Start MCP server — 7 tools for LLM code navigation
    #[command(
        long_about = "Start the Model Context Protocol (MCP) server over stdio.\n\n\
            Exposes 7 tools that LLM agents (Claude, GPT, etc.) can call for O(1) \
            symbol lookup, dependency graphs, and surgical source reads — all without \
            reading entire files.",
        after_long_help = cstr!(
            r#"<bold><underline>Tools</underline></bold>
  <bold>fmm_lookup_export</bold>    Find which file defines a symbol — O(1)
  <bold>fmm_read_symbol</bold>      Extract exact source lines by symbol name
  <bold>fmm_dependency_graph</bold>  Upstream deps + downstream dependents
  <bold>fmm_file_outline</bold>     Table of contents with line ranges
  <bold>fmm_list_exports</bold>     Search exports by pattern (fuzzy)
  <bold>fmm_file_info</bold>        Structural profile without reading source
  <bold>fmm_search</bold>           Multi-criteria AND queries

<bold><underline>Setup</underline></bold>
  <dim>$</dim> <bold>fmm init --mcp</bold>                      <dim># Add to .mcp.json</dim>

  <dim>Or manually add to .mcp.json:</dim>
  <dim>{ "mcpServers": { "fmm": { "command": "npx", "args": ["frontmatter-matters", "mcp"] } } }</dim>

<bold><underline>Notes</underline></bold>
  Communicates over stdio using the MCP JSON-RPC protocol.
  Requires sidecars to be generated first ('fmm generate').
  88-97% token reduction vs reading source files directly."#),
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
