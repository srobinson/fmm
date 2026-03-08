use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use color_print::cstr;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

use crate::config::Config;

mod commands;
mod glossary;
pub mod init;
mod search;
mod sidecar;
mod status;
mod watch;

mod help_text;
use help_text::{HELP_TEMPLATE, LONG_ABOUT, LONG_HELP, SHORT_HELP};

mod generated_help;

pub use commands::{deps, exports, lookup, ls, outline, read_symbol};
pub use glossary::glossary;
pub use init::init;
pub use init::init_skill;
pub use search::search;
pub use sidecar::{clean, generate, validate};
pub use status::status;
pub use watch::watch;

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
    /// Index source files into the SQLite database
    #[command(
        alias = "update",
        long_about = "Index source files into the SQLite database.\n\n\
            Captures each file's exports, imports, dependencies, and line count. \
            New files are indexed; existing entries are updated only when the source \
            file has changed (mtime-based incremental).",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm generate</bold>             <dim># All files in current directory</dim>
  <dim>$</dim> <bold>fmm generate src/</bold>         <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate src/ lib/</bold>    <dim># Multiple directories</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>           <dim># Dry run — preview without writing</dim>
  <dim>$</dim> <bold>fmm generate --force</bold>       <dim># Regenerate all, even if unchanged</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm generate</bold>                       <dim># All files in current directory</dim>
  <dim>$</dim> <bold>fmm generate src/</bold>                   <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate src/ lib/</bold>              <dim># Multiple directories</dim>
  <dim>$</dim> <bold>fmm generate src/auth.ts src/db.ts</bold>  <dim># Multiple files</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>                     <dim># Dry run — preview without writing</dim>
  <dim>$</dim> <bold>fmm generate --force</bold>                <dim># Regenerate all, even if unchanged</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>        <dim># Generate then verify</dim>

<bold><underline>Notes</underline></bold>
  Indexes new files and updates stale entries in a single pass.
  Unchanged files are skipped (mtime check) — no unnecessary work.
  Respects .gitignore and .fmmignore for file exclusion.
  Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C++, C#, Ruby."#),
    )]
    Generate {
        /// Paths to files or directories (defaults to current directory)
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Show what would be created/updated without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Re-index all files, bypassing mtime comparison
        #[arg(short, long)]
        force: bool,

        /// Suppress progress bars — print only the final summary line
        #[arg(short = 'q', long)]
        quiet: bool,
    },

    /// Check the index is current (CI-friendly, exit 1 if stale)
    #[command(
        long_about = "Validate that all source files are up to date in the index.\n\n\
            Returns exit code 0 if the index is current, or 1 if any files are stale or \
            missing. Designed for CI pipelines — add to your pre-commit hooks or GitHub Actions.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>             <dim># Check all indexed files</dim>
  <dim>$</dim> <bold>fmm validate src/</bold>         <dim># Check specific directory</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>                       <dim># Check all indexed files</dim>
  <dim>$</dim> <bold>fmm validate src/</bold>                   <dim># Check specific directory</dim>

  <dim># CI pipeline:</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>        <dim># Index then verify</dim>

  <dim># GitHub Actions step:</dim>
  <dim>- run: npx frontmatter-matters validate</dim>

  <dim># Pre-commit hook (.husky/pre-commit):</dim>
  <dim>fmm validate || (echo "Stale index — run 'fmm generate'" && exit 1)</dim>

<bold><underline>Notes</underline></bold>
  Exit code 0: index is current.
  Exit code 1: stale or un-indexed files found.
  Run 'fmm generate' to update the index."#),
    )]
    Validate {
        /// Paths to files or directories (defaults to current directory)
        #[arg(default_value = ".")]
        paths: Vec<String>,
    },

    /// Remove the fmm index database
    #[command(
        long_about = "Remove the fmm index database from the project.\n\n\
            Clears all indexed data from .fmm.db. Use --db to delete the database file \
            entirely. Use this to reset the index or cleanly uninstall fmm.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm clean</bold>               <dim># Clear all indexed data</dim>
  <dim>$</dim> <bold>fmm clean --db</bold>           <dim># Delete .fmm.db file entirely</dim>
  <dim>$</dim> <bold>fmm clean -n</bold>             <dim># Preview what would be removed</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm clean</bold>                          <dim># Clear all indexed data from .fmm.db</dim>
  <dim>$</dim> <bold>fmm clean --db</bold>                     <dim># Delete the .fmm.db file entirely</dim>
  <dim>$</dim> <bold>fmm clean -n</bold>                        <dim># Preview what would be removed</dim>

<bold><underline>Notes</underline></bold>
  Removes indexed data only — source files are never touched.
  Safe to re-run: 'fmm generate' recreates everything from source."#),
    )]
    Clean {
        /// Paths to files or directories (defaults to current directory)
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Show what would be removed without deleting files
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Delete the .fmm.db file entirely instead of just clearing its contents
        #[arg(long = "db")]
        delete_db: bool,
    },

    /// Watch source files and update the index on change
    #[command(
        long_about = "Watch source files for changes and update the index automatically.\n\n\
            Runs an initial generate pass on startup, then watches for file create, modify, and \
            delete events. Debounces rapid changes (default: 300ms) to avoid redundant work.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm watch</bold>               <dim># Watch current directory</dim>
  <dim>$</dim> <bold>fmm watch src/</bold>           <dim># Watch specific directory</dim>
  <dim>$</dim> <bold>fmm watch --debounce 500</bold> <dim># Custom debounce (500ms)</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm watch</bold>                          <dim># Watch current directory</dim>
  <dim>$</dim> <bold>fmm watch src/</bold>                      <dim># Watch specific directory</dim>
  <dim>$</dim> <bold>fmm watch --debounce 500</bold>            <dim># Custom debounce (500ms)</dim>

<bold><underline>Notes</underline></bold>
  Runs 'fmm generate' on startup to ensure the index is current.
  Only prints when a file is re-indexed — quiet by default.
  Respects .gitignore and .fmmignore for file exclusion.
  Press Ctrl+C to stop watching."#),
    )]
    Watch {
        /// Path to directory to watch
        #[arg(default_value = ".")]
        path: String,

        /// Debounce delay in milliseconds
        #[arg(long, default_value = "300")]
        debounce: u64,
    },

    /// Set up config, Claude skill, and MCP server
    #[command(
        long_about = "Set up fmm in the current project.\n\n\
            Creates .fmmrc.toml config and configures the MCP server in .claude/fmm.local.json. \
            The Claude Code skill is opt-in via --skill (avoid creating a project-level \
            .claude/ directory which overrides global plugin config). \
            Run with no flags for the standard setup, or use flags to install individual components.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                 <dim># Config + MCP + index source files</dim>
  <dim>$</dim> <bold>fmm init --skill</bold>          <dim># Also install Claude Code skill</dim>
  <dim>$</dim> <bold>fmm init --mcp</bold>            <dim># MCP server config only</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                           <dim># Config + MCP + index source files</dim>
  <dim>$</dim> <bold>fmm init --skill</bold>                    <dim># Also install Claude Code skill (.claude/)</dim>
  <dim>$</dim> <bold>fmm init --all</bold>                      <dim># Everything including skill</dim>
  <dim>$</dim> <bold>fmm init --mcp</bold>                      <dim># MCP server config only</dim>
  <dim>$</dim> <bold>fmm init --all --no-generate</bold>        <dim># Config files only, skip indexing</dim>

<bold><underline>What gets created</underline></bold>
  <bold>.fmmrc.toml</bold>                           Project configuration
  <bold>.claude/fmm.local.json</bold>                 MCP server config (gitignored, local scope)
  <bold>.claude/skills/fmm-navigate/SKILL.md</bold>   Claude Code skill (opt-in via --skill)

<bold><underline>Notes</underline></bold>
  Safe to re-run — existing files are not overwritten.
  MCP config uses .claude/fmm.local.json — gitignored, per-user, no merge conflicts.
  The --skill flag creates .claude/skills/ which may override global plugin skills.
  If using the helioy plugin globally, skip --skill to inherit skills from the plugin.
  The MCP config enables 9 tools for O(1) symbol lookup and navigation."#),
    )]
    Init {
        /// Install Claude Code skill (.claude/skills/fmm-navigate.md) — opt-in, creates project .claude/ dir
        #[arg(long)]
        skill: bool,

        /// Install MCP server config only (.claude/fmm.local.json)
        #[arg(long)]
        mcp: bool,

        /// Install all integrations (non-interactive)
        #[arg(long)]
        all: bool,

        /// Skip auto-indexing (config files only)
        #[arg(long)]
        no_generate: bool,
    },

    /// Show config, supported languages, and workspace stats
    #[command(
        long_about = "Display the current fmm configuration, supported languages, and \
            workspace statistics including source file and index counts.",
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm status</bold>                         <dim># Show config and stats</dim>

<bold><underline>Notes</underline></bold>
  Shows: config file location, supported languages, indexed file counts.
  Useful for verifying fmm is set up correctly in a project."#),
    )]
    Status,

    /// Query the index — O(1) export lookup, dependency graphs, LOC filters
    #[command(
        long_about = generated_help::SEARCH_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm search store</bold>            <dim># Smart search across everything</dim>
  <dim>$</dim> <bold>fmm search -e createStore</bold>   <dim># Export lookup (exact + fuzzy)</dim>
  <dim>$</dim> <bold>fmm search -i react</bold>          <dim># Files importing react</dim>
  <dim>$</dim> <bold>fmm search -l ">500"</bold>        <dim># Large files</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim># Smart search (searches everything, best matches first):</dim>
  <dim>$</dim> <bold>fmm search store</bold>                   <dim># Exports, files, and imports matching "store"</dim>
  <dim>$</dim> <bold>fmm search createStore</bold>              <dim># Exact export match ranked first</dim>
  <dim>$</dim> <bold>fmm search auth</bold>                     <dim># Find auth-related symbols and files</dim>

  <dim># Export lookup (exact O(1), then fuzzy substring):</dim>
  <dim>$</dim> <bold>fmm search --export createStore</bold>    <dim># Exact match</dim>
  <dim>$</dim> <bold>fmm search --export store</bold>           <dim># Fuzzy: createStore, useStore, StoreProvider</dim>
  <dim>$</dim> <bold>fmm search --export STORE</bold>           <dim># Case-insensitive fuzzy match</dim>

  <dim># Import analysis:</dim>
  <dim>$</dim> <bold>fmm search --imports react</bold>          <dim># All files importing react</dim>
  <dim>$</dim> <bold>fmm search --imports crypto</bold>         <dim># Find crypto usage across codebase</dim>

  <dim># Dependency graph (impact analysis):</dim>
  <dim>$</dim> <bold>fmm search --depends-on src/auth.ts</bold> <dim># What breaks if auth changes?</dim>
  <dim>$</dim> <bold>fmm search --depends-on src/db.ts</bold>   <dim># Downstream dependents of db</dim>

  <dim># Line count filters:</dim>
  <dim>$</dim> <bold>fmm search --loc ">>500"</bold>            <dim># Large files (over 500 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc "<<50"</bold>             <dim># Small files (under 50 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc ">=100"</bold>            <dim># Files with 100+ lines</dim>

  <dim># Combined filters (AND logic):</dim>
  <dim>$</dim> <bold>fmm search --imports react --loc ">>200"</bold>  <dim># Large React files</dim>

  <dim># Structured output:</dim>
  <dim>$</dim> <bold>fmm search store --json</bold>             <dim># JSON for scripting/piping</dim>
  <dim>$</dim> <bold>fmm search --export App --json</bold>      <dim># JSON for scripting/piping</dim>
  <dim>$</dim> <bold>fmm search --json</bold>                   <dim># All indexed files as JSON</dim>

<bold><underline>Notes</underline></bold>
  Bare search (<bold>fmm search <<term>></bold>) is the fastest way to find anything.
  Export lookup is O(1) — uses a pre-built reverse index, not file scanning.
  Flags narrow search to one dimension. Without flags, searches everything.
  Use --json for machine-readable output (piping, scripts, CI)."#),
    )]
    Search {
        /// Search term — searches exports, files, and imports (smart ranking)
        #[arg(value_name = "TERM")]
        term: Option<String>,

        /// Find file by export name (exact O(1) + fuzzy substring)
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

        /// Scope --export results to a directory prefix (e.g. packages/)
        #[arg(long = "dir")]
        dir: Option<String>,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Show all definitions of an export and which files use it
    #[command(
        long_about = generated_help::GLOSSARY_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm glossary run_dispatch</bold>              <dim># Exact symbol lookup (source mode)</dim>
  <dim>$</dim> <bold>fmm glossary config</bold>                    <dim># All Config, loadConfig, AppConfig, ...</dim>
  <dim>$</dim> <bold>fmm glossary run_dispatch --mode tests</bold> <dim># What tests cover this symbol?</dim>
  <dim>$</dim> <bold>fmm glossary config --mode all</bold>         <dim># Source + tests combined</dim>
  <dim>$</dim> <bold>fmm glossary config --limit 20</bold>         <dim># Limit results</dim>
  <dim>$</dim> <bold>fmm glossary config --json</bold>             <dim># JSON output for scripting</dim>"#),
    )]
    Glossary {
        /// Symbol name or substring pattern (case-insensitive)
        #[arg(value_name = "PATTERN")]
        pattern: Option<String>,

        /// Filter mode: source (default, no tests), tests (test coverage only), all (unfiltered)
        #[arg(long, value_name = "MODE", default_value = "source", value_parser = ["source", "tests", "all"])]
        mode: String,

        /// Maximum number of entries returned (default: 10)
        #[arg(long)]
        limit: Option<usize>,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Find where a symbol is defined — O(1) lookup
    #[command(
        long_about = generated_help::LOOKUP_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm lookup Injector</bold>           <dim># Find symbol definition</dim>
  <dim>$</dim> <bold>fmm lookup createPipeline</bold>     <dim># Any exported name</dim>
  <dim>$</dim> <bold>fmm lookup Injector --json</bold>    <dim># JSON output</dim>"#),
    )]
    Lookup {
        /// Symbol name to look up (exact match; use 'fmm exports <term>' for fuzzy)
        #[arg(value_name = "SYMBOL")]
        symbol: String,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Extract exact source for a symbol or method
    #[command(
        name = "read",
        long_about = generated_help::READ_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm read Injector</bold>                       <dim># Full class source</dim>
  <dim>$</dim> <bold>fmm read Injector.loadInstance</bold>          <dim># Single method</dim>
  <dim>$</dim> <bold>fmm read Injector --no-truncate</bold>        <dim># Bypass 10KB cap</dim>
  <dim>$</dim> <bold>fmm read Injector --line-numbers</bold>       <dim># With absolute line numbers</dim>
  <dim>$</dim> <bold>fmm read createStore --json</bold>             <dim># JSON output</dim>"#),
    )]
    Read {
        /// Symbol name (or ClassName.method for a specific public method)
        #[arg(value_name = "SYMBOL")]
        symbol: String,

        /// Return full source, bypassing the 10KB truncation cap
        #[arg(long = "no-truncate")]
        no_truncate: bool,

        /// Prepend absolute line numbers to each source line
        #[arg(long = "line-numbers")]
        line_numbers: bool,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Show dependency graph for a file
    #[command(
        long_about = generated_help::DEPS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm deps src/injector.ts</bold>                           <dim># Direct deps (depth=1)</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --depth 2</bold>                <dim># Transitive (2 hops)</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --depth -1</bold>               <dim># Full closure</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --filter source</bold>          <dim># Exclude test files from downstream</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --filter tests</bold>           <dim># Only test files in downstream</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --json</bold>                    <dim># JSON output</dim>"#),
    )]
    Deps {
        /// Source file path (relative to project root, as indexed by fmm)
        #[arg(value_name = "FILE")]
        file: String,

        /// Traversal depth (1 = direct deps only, -1 = full closure)
        #[arg(long, default_value = "1")]
        depth: i32,

        /// Filter upstream/downstream by file type: all (default), source (exclude tests), tests (only tests)
        #[arg(long, default_value = "all", value_parser = ["all", "source", "tests"])]
        filter: String,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Show file table-of-contents with export line ranges
    #[command(
        long_about = generated_help::OUTLINE_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm outline src/injector.ts</bold>                  <dim># All exports + line ranges</dim>
  <dim>$</dim> <bold>fmm outline src/injector.ts --include-private</bold> <dim># Include private members</dim>
  <dim>$</dim> <bold>fmm outline src/injector.ts --json</bold>            <dim># JSON output</dim>"#),
    )]
    Outline {
        /// Source file path (relative to project root)
        #[arg(value_name = "FILE")]
        file: String,

        /// Include private/protected methods and fields under each class
        #[arg(long = "include-private")]
        include_private: bool,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// List indexed files under a directory
    #[command(
        long_about = generated_help::LS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm ls</bold>                                 <dim># All indexed files (sorted by LOC)</dim>
  <dim>$</dim> <bold>fmm ls src/</bold>                            <dim># Files under src/</dim>
  <dim>$</dim> <bold>fmm ls --sort-by downstream</bold>            <dim># Most-imported files first (pre-refactoring)</dim>
  <dim>$</dim> <bold>fmm ls --sort-by loc</bold>                   <dim># Heaviest files first</dim>
  <dim>$</dim> <bold>fmm ls --sort-by exports</bold>               <dim># Most exports first</dim>
  <dim>$</dim> <bold>fmm ls --sort-by name</bold>                  <dim># Alphabetical</dim>
  <dim>$</dim> <bold>fmm ls --sort-by modified</bold>              <dim># Most recently changed first</dim>
  <dim>$</dim> <bold>fmm ls --group-by subdir</bold>               <dim># Directory rollup (file count + LOC)</dim>
  <dim>$</dim> <bold>fmm ls --filter source</bold>                 <dim># Source files only (no tests)</dim>
  <dim>$</dim> <bold>fmm ls --pattern "*.ts"</bold>                <dim># Filter by filename glob</dim>
  <dim>$</dim> <bold>fmm ls --limit 20 --offset 20</bold>          <dim># Pagination</dim>
  <dim>$</dim> <bold>fmm ls src/ --json</bold>                     <dim># JSON output</dim>"#),
    )]
    Ls {
        /// Directory prefix to filter (e.g. src/, packages/core/)
        #[arg(value_name = "DIR")]
        directory: Option<String>,

        /// Glob pattern to filter by filename (e.g. '*.ts', '*.rs', 'test_*')
        #[arg(long)]
        pattern: Option<String>,

        /// Sort field: loc (default), name, exports, downstream, modified
        #[arg(long = "sort-by", default_value = "loc", value_parser = ["name", "loc", "exports", "downstream", "modified"])]
        sort_by: String,

        /// Sort order: asc or desc (default depends on sort-by)
        #[arg(long, value_parser = ["asc", "desc"])]
        order: Option<String>,

        /// Collapse files into directory buckets (subdir: group by immediate subdirectory)
        #[arg(long = "group-by", value_parser = ["subdir"])]
        group_by: Option<String>,

        /// File type filter: all (default), source (exclude tests), tests (only tests)
        #[arg(long, default_value = "all", value_parser = ["all", "source", "tests"])]
        filter: String,

        /// Maximum number of files to return (default: 200)
        #[arg(long)]
        limit: Option<usize>,

        /// Number of files to skip (default: 0) — use for pagination
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Search exports by pattern (substring or regex, auto-detected)
    #[command(
        long_about = generated_help::EXPORTS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm exports</bold>                              <dim># All exports (grouped by file)</dim>
  <dim>$</dim> <bold>fmm exports Module</bold>                       <dim># Substring match "Module"</dim>
  <dim>$</dim> <bold>fmm exports create</bold>                       <dim># All exports containing "create"</dim>
  <dim>$</dim> <bold>fmm exports '^handle'</bold>                    <dim># Regex: exports starting with "handle"</dim>
  <dim>$</dim> <bold>fmm exports 'Service$'</bold>                   <dim># Regex: exports ending in "Service"</dim>
  <dim>$</dim> <bold>fmm exports '^[A-Z]'</bold>                     <dim># Regex: PascalCase exports only</dim>
  <dim>$</dim> <bold>fmm exports Module --dir packages/core/</bold>   <dim># Scoped to directory</dim>
  <dim>$</dim> <bold>fmm exports Module --limit 50 --offset 50</bold> <dim># Pagination</dim>
  <dim>$</dim> <bold>fmm exports Module --json</bold>                 <dim># JSON output</dim>"#),
    )]
    Exports {
        /// Pattern to filter exports — substring (case-insensitive) or regex (auto-detected when metacharacters present)
        #[arg(value_name = "PATTERN")]
        pattern: Option<String>,

        /// Scope results to a directory prefix (e.g. packages/core/)
        #[arg(long = "dir")]
        dir: Option<String>,

        /// Maximum number of results (default: 200)
        #[arg(long)]
        limit: Option<usize>,

        /// Number of results to skip (default: 0) — use for pagination
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Output as JSON
        #[arg(short = 'j', long = "json")]
        json: bool,
    },

    /// Start MCP server — 9 tools for LLM code navigation
    #[command(
        long_about = "Start the Model Context Protocol (MCP) server over stdio.\n\n\
            Exposes 9 tools that LLM agents (Claude, GPT, etc.) can call for O(1) \
            symbol lookup, dependency graphs, and surgical source reads — all without \
            reading entire files.",
        after_long_help = cstr!(
            r#"<bold><underline>Tools</underline></bold>
  <bold>fmm_lookup_export</bold>    Find which file defines a symbol — O(1)
  <bold>fmm_read_symbol</bold>      Extract exact source; use ClassName.method for public methods
  <bold>fmm_dependency_graph</bold>  local_deps, external packages, and downstream blast radius
  <bold>fmm_file_outline</bold>     Table of contents with line ranges
  <bold>fmm_list_exports</bold>     Search exports by pattern (fuzzy)
  <bold>fmm_search</bold>           Multi-criteria AND queries with relevance scoring
  <bold>fmm_list_files</bold>       List all indexed files under a directory path
  <bold>fmm_glossary</bold>         Symbol-level blast radius — all definitions + who imports each

<bold><underline>Setup</underline></bold>
  <dim>$</dim> <bold>fmm init --mcp</bold>                      <dim># Add to .claude/fmm.local.json</dim>

  <dim>Or manually add to .claude/fmm.local.json:</dim>
  <dim>{ "mcpServers": { "fmm": { "command": "fmm", "args": ["mcp"] } } }</dim>

<bold><underline>Notes</underline></bold>
  Communicates over stdio using the MCP JSON-RPC protocol.
  Requires the index to be built first ('fmm generate').
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
/// If a directory, use it directly. If a file, walk up from its parent
/// looking for project root markers (.git, .fmmrc.toml, .fmmrc.json) so that relative
/// paths in the index are consistent regardless of whether `fmm generate`
/// targets a single file or the whole repo.
/// Falls back to the file's parent directory, then CWD.
fn resolve_root(path: &str) -> Result<PathBuf> {
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    if target.is_dir() {
        Ok(target)
    } else if target.is_file() {
        match target.parent() {
            Some(parent) => Ok(find_project_root(parent).unwrap_or_else(|| parent.to_path_buf())),
            None => std::env::current_dir().context("Failed to get current directory"),
        }
    } else {
        std::env::current_dir().context("Failed to get current directory")
    }
}

/// Walk up from `start` looking for project root markers.
/// Returns the first directory containing `.git`, `.fmmrc.toml`, or `.fmmrc.json`.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists()
            || current.join(".fmmrc.toml").exists()
            || current.join(".fmmrc.json").exists()
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Returns `(kept_files, skipped_count)`.
///
/// Files exceeding `config.max_lines` are excluded from `kept_files` and
/// counted in `skipped_count`. The caller is responsible for reporting skips.
fn collect_files(path: &str, config: &Config) -> Result<(Vec<PathBuf>, usize)> {
    let path = Path::new(path);

    if path.is_file() {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if config.max_lines > 0 && !file_within_line_limit(&canonical, config.max_lines) {
            return Ok((vec![], 1));
        }
        return Ok((vec![canonical], 0));
    }

    // Pre-compile exclude glob patterns (relative to the walk root).
    let exclude_patterns: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let candidates: Vec<PathBuf> = WalkBuilder::new(path)
        .standard_filters(true)
        .add_custom_ignore_filename(".fmmignore")
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| {
            if let Some(ext) = entry.path().extension() {
                config.is_supported_language(ext.to_str().unwrap_or(""))
            } else {
                false
            }
        })
        .filter(|entry| {
            if exclude_patterns.is_empty() {
                return true;
            }
            // Normalize the entry path: strip leading "./" so patterns like
            // "vendor/**" match both "vendor/foo.js" and "./vendor/foo.js".
            let raw = entry.path().to_string_lossy();
            let rel = raw.strip_prefix("./").unwrap_or(&raw);
            !exclude_patterns.iter().any(|pat| {
                pat.matches_with(
                    rel,
                    glob::MatchOptions {
                        require_literal_separator: false,
                        ..Default::default()
                    },
                )
            })
        })
        .map(|entry| {
            entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf())
        })
        .collect();

    if config.max_lines == 0 {
        return Ok((candidates, 0));
    }

    let (files, skipped): (Vec<PathBuf>, Vec<PathBuf>) = candidates
        .into_iter()
        .partition(|canonical| file_within_line_limit(canonical, config.max_lines));
    let skip_count = skipped.len();
    Ok((files, skip_count))
}

/// Returns true if the file has at most `max_lines` lines.
///
/// Uses a byte-size lower bound to avoid reading most files: any file with
/// fewer bytes than `max_lines` cannot possibly have more than `max_lines`
/// lines (every line needs at least one byte for the newline character).
/// Files that exceed this threshold are read as raw bytes and their newlines
/// counted — no UTF-8 decoding required.
fn file_within_line_limit(path: &Path, max_lines: usize) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return true; // cannot stat: include and let the parser decide
    };
    // Fast path: if file_bytes <= max_lines, it cannot have more than max_lines
    // lines regardless of content.
    if meta.len() <= max_lines as u64 {
        return true;
    }
    // Slow path: count newlines by reading raw bytes (no UTF-8 overhead).
    let Ok(bytes) = std::fs::read(path) else {
        return true; // cannot read: include and let the parser decide
    };
    let line_count = bytecount_newlines(&bytes);
    line_count <= max_lines
}

/// Count lines in a byte slice by counting `\n` characters.
///
/// Each `\n` terminates exactly one line. A file with content but no trailing
/// newline has one more line than it has newlines (the unterminated final line).
///
/// Examples: `""` → 0, `"hello"` → 1, `"a\nb"` → 2, `"a\nb\n"` → 2.
#[inline]
fn bytecount_newlines(bytes: &[u8]) -> usize {
    let newlines = bytes.iter().filter(|&&b| b == b'\n').count();
    if bytes.is_empty() {
        0
    } else if bytes.last() == Some(&b'\n') {
        // Every newline terminates exactly one line.
        newlines
    } else {
        // Final line has no terminating newline — add 1.
        newlines + 1
    }
}

fn collect_files_multi(paths: &[String], config: &Config) -> Result<(Vec<PathBuf>, usize)> {
    let mut all_files = Vec::new();
    let mut total_skipped = 0usize;
    for path in paths {
        let (files, skipped) = collect_files(path, config)?;
        all_files.extend(files);
        total_skipped += skipped;
    }
    all_files.sort();
    all_files.dedup();
    Ok((all_files, total_skipped))
}

/// Resolve root from multiple paths: common ancestor if all exist, else CWD.
fn resolve_root_multi(paths: &[String]) -> Result<PathBuf> {
    if paths.len() == 1 {
        return resolve_root(&paths[0]);
    }

    let resolved: Vec<PathBuf> = paths.iter().filter_map(|p| resolve_root(p).ok()).collect();

    if resolved.is_empty() {
        return std::env::current_dir().context("Failed to get current directory");
    }

    // Find common ancestor
    let mut ancestor = resolved[0].clone();
    for path in &resolved[1..] {
        while !path.starts_with(&ancestor) {
            if !ancestor.pop() {
                return std::env::current_dir().context("Failed to get current directory");
            }
        }
    }
    Ok(ancestor)
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
    fn resolve_root_with_file_finds_project_root() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src").join("deep");
        std::fs::create_dir_all(&src).unwrap();
        // Place a .git marker at the tmp root
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let file_path = src.join("example.ts");
        std::fs::write(&file_path, "export const x = 1;").unwrap();

        let result = resolve_root(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_root_with_file_falls_back_to_parent_without_markers() {
        let tmp = TempDir::new().unwrap();
        // No .git or .fmmrc.json in any ancestor within tmp
        let file_path = tmp.path().join("example.ts");
        std::fs::write(&file_path, "export const x = 1;").unwrap();

        let result = resolve_root(file_path.to_str().unwrap()).unwrap();
        // Without project markers, walks up and may find a .git above tmp,
        // or falls back to the file's parent
        assert!(result.is_dir());
        // The file's parent should be an ancestor of (or equal to) the result
        let parent = file_path.parent().unwrap().canonicalize().unwrap();
        assert!(parent.starts_with(&result) || result == parent);
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
        let (files, _) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();

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
        let (files, _) = collect_files(file_path.to_str().unwrap(), &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].is_absolute());
        assert_eq!(files[0], file_path.canonicalize().unwrap());
    }

    // bytecount_newlines unit tests

    #[test]
    fn bytecount_newlines_empty() {
        assert_eq!(bytecount_newlines(b""), 0);
    }

    #[test]
    fn bytecount_newlines_single_line_no_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello"), 1);
    }

    #[test]
    fn bytecount_newlines_single_line_with_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello\n"), 1);
    }

    #[test]
    fn bytecount_newlines_two_lines_with_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello\nworld\n"), 2);
    }

    #[test]
    fn bytecount_newlines_two_lines_no_trailing_newline() {
        // This was the previously failing case: 1 newline → max(1,1) = 1, wrong.
        assert_eq!(bytecount_newlines(b"hello\nworld"), 2);
    }

    #[test]
    fn bytecount_newlines_only_newlines() {
        assert_eq!(bytecount_newlines(b"\n\n\n"), 3);
    }

    // collect_files respects max_lines

    #[test]
    fn collect_files_excludes_file_exceeding_max_lines() {
        let tmp = TempDir::new().unwrap();
        // Write a file with 6 lines (3 newline-terminated + 1 content line with no trailing \n
        // → easier: just write 5 lines with trailing newlines).
        let big: String = (0..5).map(|i| format!("line{}\n", i)).collect();
        std::fs::write(tmp.path().join("big.ts"), &big).unwrap();
        std::fs::write(tmp.path().join("small.ts"), "export const x = 1;\n").unwrap();

        let config = Config {
            max_lines: 3,
            ..Default::default()
        }; // big.ts has 5 lines, small.ts has 1

        let (files, skipped) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();
        assert_eq!(files.len(), 1, "only small.ts should be collected");
        assert_eq!(skipped, 1, "big.ts should be counted as skipped");
        assert!(files[0].to_string_lossy().contains("small.ts"));
    }

    #[test]
    fn collect_files_max_lines_zero_disables_limit() {
        let tmp = TempDir::new().unwrap();
        let big: String = (0..200).map(|i| format!("line{}\n", i)).collect();
        std::fs::write(tmp.path().join("big.ts"), &big).unwrap();

        let config = Config {
            max_lines: 0,
            ..Default::default()
        }; // 0 disables the limit

        let (files, skipped) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(skipped, 0);
    }
}
