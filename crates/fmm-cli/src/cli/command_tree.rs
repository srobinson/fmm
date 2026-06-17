use clap::Subcommand;
use color_print::cstr;

use super::commands::{
    CleanCommandArgs, CompletionsCommandArgs, CyclesCommandArgs, DepsCommandArgs, DupesCommandArgs,
    ExportsCommandArgs, GenerateCommandArgs, GlossaryCommandArgs, InitCommandArgs,
    LookupCommandArgs, LsCommandArgs, OutlineCommandArgs, ReadCommandArgs, SearchCommandArgs,
    SimilarCommandArgs, ValidateCommandArgs, WatchCommandArgs,
};
use super::generated_help;

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
  <dim>$</dim> <bold>fmm generate crates/fmm-core/src</bold> <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate crates/fmm-core/src crates/fmm-cli/src</bold> <dim># Multiple directories</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>           <dim># Dry run — preview without writing</dim>
  <dim>$</dim> <bold>fmm generate --force</bold>       <dim># Regenerate all, even if unchanged</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm generate</bold>                       <dim># All files in current directory</dim>
  <dim>$</dim> <bold>fmm generate crates/fmm-core/src</bold>   <dim># Specific directory only</dim>
  <dim>$</dim> <bold>fmm generate crates/fmm-core/src crates/fmm-cli/src</bold> <dim># Multiple directories</dim>
  <dim>$</dim> <bold>fmm generate crates/fmm-cli/src/main.rs crates/fmm-core/src/lib.rs</bold> <dim># Multiple files</dim>
  <dim>$</dim> <bold>fmm generate -n</bold>                     <dim># Dry run — preview without writing</dim>
  <dim>$</dim> <bold>fmm generate --force</bold>                <dim># Regenerate all, even if unchanged</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>        <dim># Generate then verify</dim>

<bold><underline>Notes</underline></bold>
  Indexes new files and updates stale entries in a single pass.
  Unchanged files are skipped (mtime check) — no unnecessary work.
  Respects .gitignore and .fmmignore for file exclusion.
  Supports: TypeScript, JavaScript, Python, Rust."#),
    )]
    Generate(GenerateCommandArgs),

    /// Check the index is current (CI-friendly, exit 1 if stale)
    #[command(
        long_about = "Validate that all source files are up to date in the index.\n\n\
            Returns exit code 0 if the index is current, or 1 if any files are stale or \
            missing. Designed for CI pipelines — add to your pre-commit hooks or GitHub Actions.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>             <dim># Check all indexed files</dim>
  <dim>$</dim> <bold>fmm validate crates/fmm-core/src</bold> <dim># Check specific directory</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm validate</bold>                       <dim># Check all indexed files</dim>
  <dim>$</dim> <bold>fmm validate crates/fmm-core/src</bold>   <dim># Check specific directory</dim>

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
    Validate(ValidateCommandArgs),

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
    Clean(CleanCommandArgs),

    /// Watch source files and update the index on change
    #[command(
        long_about = "Watch source files for changes and update the index automatically.\n\n\
            Runs an initial generate pass on startup, then watches for file create, modify, and \
            delete events. Debounces rapid changes (default: 300ms) to avoid redundant work.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm watch</bold>               <dim># Watch current directory</dim>
  <dim>$</dim> <bold>fmm watch crates/fmm-core/src</bold> <dim># Watch specific directory</dim>
  <dim>$</dim> <bold>fmm watch --debounce 500</bold> <dim># Custom debounce (500ms)</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm watch</bold>                          <dim># Watch current directory</dim>
  <dim>$</dim> <bold>fmm watch crates/fmm-core/src</bold>        <dim># Watch specific directory</dim>
  <dim>$</dim> <bold>fmm watch --debounce 500</bold>            <dim># Custom debounce (500ms)</dim>

<bold><underline>Notes</underline></bold>
  Runs 'fmm generate' on startup to ensure the index is current.
  Only prints when a file is re-indexed — quiet by default.
  Respects .gitignore and .fmmignore for file exclusion.
  Press Ctrl+C to stop watching."#),
    )]
    Watch(WatchCommandArgs),

    /// Set up fmm in the current project
    #[command(
        long_about = "Set up fmm in the current project.\n\n\
            Creates .fmmrc.toml config and indexes source files. \
            Safe to re-run: existing config is not overwritten.",
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                 <dim># Create config + index source files</dim>
  <dim>$</dim> <bold>fmm init --force</bold>         <dim># Overwrite existing config</dim>
  <dim>$</dim> <bold>fmm init --no-generate</bold>   <dim># Config only, skip indexing</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                           <dim># Create config + index source files</dim>
  <dim>$</dim> <bold>fmm init --force</bold>                   <dim># Overwrite existing .fmmrc.toml</dim>
  <dim>$</dim> <bold>fmm init --no-generate</bold>              <dim># Config only, skip indexing</dim>

<bold><underline>What gets created</underline></bold>
  <bold>.fmmrc.toml</bold>                           Project configuration (optional, defaults apply)

<bold><underline>Notes</underline></bold>
  Safe to re-run: existing .fmmrc.toml is not overwritten unless --force is used.
  .fmmrc.toml is optional: delete it to use built-in defaults."#),
    )]
    Init(InitCommandArgs),

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
  <dim>$</dim> <bold>fmm search -e ParserRegistry</bold> <dim># Export lookup (exact + fuzzy)</dim>
  <dim>$</dim> <bold>fmm search -i anyhow</bold>         <dim># Files importing anyhow</dim>
  <dim>$</dim> <bold>fmm search -l ">500"</bold>        <dim># Large files</dim>"#),
        after_long_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>

  <dim># Smart search (searches everything, best matches first):</dim>
  <dim>$</dim> <bold>fmm search parser</bold>                  <dim># Exports, files, and imports matching "parser"</dim>
  <dim>$</dim> <bold>fmm search ParserRegistry</bold>          <dim># Exact export match ranked first</dim>
  <dim>$</dim> <bold>fmm search config</bold>                  <dim># Find config-related symbols and files</dim>

  <dim># Export lookup (exact O(1), then fuzzy substring):</dim>
  <dim>$</dim> <bold>fmm search --export ParserRegistry</bold> <dim># Exact match</dim>
  <dim>$</dim> <bold>fmm search --export parser</bold>         <dim># Fuzzy: ParserRegistry, Parser, CParser</dim>
  <dim>$</dim> <bold>fmm search --export CONFIG</bold>         <dim># Case-insensitive fuzzy match</dim>

  <dim># Import analysis:</dim>
  <dim>$</dim> <bold>fmm search --imports anyhow</bold>        <dim># All files importing anyhow</dim>
  <dim>$</dim> <bold>fmm search --imports serde</bold>         <dim># Find serde usage across codebase</dim>

  <dim># Dependency graph (impact analysis):</dim>
  <dim>$</dim> <bold>fmm search --depends-on crates/fmm-core/src/parser/mod.rs</bold> <dim># What breaks if parser changes?</dim>
  <dim>$</dim> <bold>fmm search --depends-on crates/fmm-core/src/config/mod.rs</bold> <dim># Downstream dependents of config</dim>

  <dim># Line count filters:</dim>
  <dim>$</dim> <bold>fmm search --loc ">500"</bold>             <dim># Large files (over 500 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc "<<50"</bold>             <dim># Small files (under 50 lines)</dim>
  <dim>$</dim> <bold>fmm search --loc ">=100"</bold>            <dim># Files with 100+ lines</dim>
  <dim>$</dim> <bold>fmm search --min-loc 100 --max-loc 500</bold> <dim># Files in a LOC range</dim>
  <dim>$</dim> <bold>fmm search parser --limit 5</bold>         <dim># Cap fuzzy export matches</dim>

  <dim># Combined filters (AND logic):</dim>
  <dim>$</dim> <bold>fmm search --imports anyhow --loc ">200"</bold> <dim># Large anyhow users</dim>

  <dim># Structured output:</dim>
  <dim>$</dim> <bold>fmm search parser --json</bold>            <dim># JSON for scripting/piping</dim>
  <dim>$</dim> <bold>fmm search --export ParserRegistry --json</bold> <dim># JSON for scripting/piping</dim>
  <dim>$</dim> <bold>fmm search --json</bold>                   <dim># All indexed files as JSON</dim>

<bold><underline>Notes</underline></bold>
  Bare search (<bold>fmm search TERM</bold>) is the fastest way to find anything.
  Export lookup is O(1) — uses a pre-built reverse index, not file scanning.
  Flags narrow search to one dimension. Without flags, searches everything.
  Use --json for machine-readable output (piping, scripts, CI)."#),
    )]
    Search(SearchCommandArgs),

    /// Show all definitions of an export and which files use it
    #[command(
        long_about = generated_help::GLOSSARY_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm glossary run_dispatch</bold>              <dim># Exact symbol lookup (source mode)</dim>
  <dim>$</dim> <bold>fmm glossary config</bold>                    <dim># All Config, loadConfig, AppConfig, ...</dim>
  <dim>$</dim> <bold>fmm glossary run_dispatch --mode tests</bold> <dim># What tests cover this symbol?</dim>
  <dim>$</dim> <bold>fmm glossary scheduleUpdate --precision call-site</bold> <dim># Confirm direct callers</dim>
  <dim>$</dim> <bold>fmm glossary config --mode all</bold>         <dim># Source + tests combined</dim>
  <dim>$</dim> <bold>fmm glossary config --limit 20</bold>         <dim># Limit results</dim>
  <dim>$</dim> <bold>fmm glossary config --no-truncate</bold>      <dim># Bypass 10KB cap</dim>
  <dim>$</dim> <bold>fmm glossary config --json</bold>             <dim># JSON output for scripting</dim>"#),
    )]
    Glossary(GlossaryCommandArgs),

    /// Find where a symbol is defined — O(1) lookup
    #[command(
        visible_alias = "lookup-export",
        long_about = generated_help::LOOKUP_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm lookup Cli</bold>                <dim># Find symbol definition</dim>
  <dim>$</dim> <bold>fmm lookup ParserRegistry</bold>     <dim># Any exported name</dim>
  <dim>$</dim> <bold>fmm lookup Cli --json</bold>          <dim># JSON output</dim>"#),
    )]
    Lookup(LookupCommandArgs),

    /// Find existing symbols similar to a probe — prevent duplication
    #[command(
        visible_alias = "find-similar",
        long_about = generated_help::SIMILAR_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm similar load_config</bold>                          <dim># Symbols like an existing one</dim>
  <dim>$</dim> <bold>fmm similar parse --signature "(Path) -> Config"</bold>  <dim># Pre-write probe</dim>
  <dim>$</dim> <bold>fmm similar load_config --limit 5 --json</bold>          <dim># JSON output</dim>"#),
    )]
    Similar(SimilarCommandArgs),

    /// Find repo wide structural duplicate candidate clusters
    #[command(
        long_about = generated_help::DUPES_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm dupes</bold>                                  <dim># Duplicate clusters in the current index</dim>
  <dim>$</dim> <bold>fmm dupes --dir crates/fmm-cli/src --kind fn</bold> <dim># Scope candidates</dim>
  <dim>$</dim> <bold>fmm dupes --min-score 0.4 --limit 5 --json</bold>   <dim># Deterministic JSON</dim>"#),
    )]
    Dupes(DupesCommandArgs),

    /// Extract exact source for a symbol or method
    #[command(
        name = "read",
        visible_alias = "read-symbol",
        long_about = generated_help::READ_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm read Commands</bold>                       <dim># Full enum source</dim>
  <dim>$</dim> <bold>fmm read ParserRegistry.get_parser</bold>      <dim># Single method</dim>
  <dim>$</dim> <bold>fmm read Commands --no-truncate</bold>         <dim># Bypass 10KB cap</dim>
  <dim>$</dim> <bold>fmm read ParserRegistry.get_parser --line-numbers</bold> <dim># With absolute line numbers</dim>
  <dim>$</dim> <bold>fmm read Cli --json</bold>                     <dim># JSON output</dim>"#),
    )]
    Read(ReadCommandArgs),

    /// Show dependency graph for a file
    #[command(
        visible_alias = "dependency-graph",
        long_about = generated_help::DEPS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs</bold>        <dim># Direct deps (depth=1)</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --depth 2</bold> <dim># Transitive (2 hops)</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --depth -1</bold> <dim># Full closure</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --filter source</bold> <dim># Exclude test files from downstream</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --filter tests</bold> <dim># Only test files in downstream</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --json</bold> <dim># JSON output</dim>"#),
    )]
    Deps(DepsCommandArgs),

    /// Report dependency cycles
    #[command(
        visible_alias = "dependency-cycles",
        long_about = generated_help::CYCLES_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm cycles</bold>                                      <dim># Runtime dependency cycles</dim>
  <dim>$</dim> <bold>fmm cycles src/app.ts</bold>                           <dim># Cycles containing one file</dim>
  <dim>$</dim> <bold>fmm cycles --edge-mode all</bold>                      <dim># Include type-only edges</dim>
  <dim>$</dim> <bold>fmm cycles --include-mod-hierarchy</bold>              <dim># Include module hierarchy edges</dim>
  <dim>$</dim> <bold>fmm cycles --explain</bold>                            <dim># Include closing edges</dim>
  <dim>$</dim> <bold>fmm cycles --filter source</bold>                      <dim># Exclude test files</dim>
  <dim>$</dim> <bold>fmm cycles --json</bold>                               <dim># JSON output</dim>"#),
    )]
    Cycles(CyclesCommandArgs),

    /// Show file table-of-contents with line ranges and density metadata
    #[command(
        visible_alias = "file-outline",
        long_about = generated_help::OUTLINE_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm outline crates/fmm-core/src/parser/mod.rs</bold> <dim># Symbols + signature/visibility/kind</dim>
  <dim>$</dim> <bold>fmm outline crates/fmm-core/src/parser/mod.rs --include-private</bold> <dim># Include private members</dim>
  <dim>$</dim> <bold>fmm outline crates/fmm-core/src/parser/mod.rs --json</bold> <dim># JSON output</dim>"#),
    )]
    Outline(OutlineCommandArgs),

    /// List indexed files under a directory
    #[command(
        visible_alias = "list-files",
        long_about = generated_help::LS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm ls</bold>                                 <dim># All indexed files (sorted by LOC)</dim>
  <dim>$</dim> <bold>fmm ls crates/fmm-core/src</bold>             <dim># Files under fmm-core source</dim>
  <dim>$</dim> <bold>fmm ls --sort-by downstream</bold>            <dim># Most-imported files first (pre-refactoring)</dim>
  <dim>$</dim> <bold>fmm ls --sort-by loc</bold>                   <dim># Heaviest files first</dim>
  <dim>$</dim> <bold>fmm ls --sort-by exports</bold>               <dim># Most exports first</dim>
  <dim>$</dim> <bold>fmm ls --sort-by name</bold>                  <dim># Alphabetical</dim>
  <dim>$</dim> <bold>fmm ls --sort-by path</bold>                  <dim># Alias for alphabetical path order</dim>
  <dim>$</dim> <bold>fmm ls --sort-by modified</bold>              <dim># Most recently changed first</dim>
  <dim>$</dim> <bold>fmm ls --group-by subdir</bold>               <dim># Directory rollup (file count + LOC)</dim>
  <dim>$</dim> <bold>fmm ls --filter source</bold>                 <dim># Source files only (no tests)</dim>
  <dim>$</dim> <bold>fmm ls --pattern "*.ts"</bold>                <dim># Filter by filename glob</dim>
  <dim>$</dim> <bold>fmm ls --limit 20 --offset 20</bold>          <dim># Pagination</dim>
  <dim>$</dim> <bold>fmm ls crates/fmm-core/src --json</bold>      <dim># JSON output</dim>"#),
    )]
    Ls(LsCommandArgs),

    /// Search exports by pattern (substring or regex, auto-detected)
    #[command(
        visible_alias = "list-exports",
        long_about = generated_help::EXPORTS_ABOUT,
        after_help = cstr!(
            r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>fmm exports</bold>                              <dim># All exports (grouped by file)</dim>
  <dim>$</dim> <bold>fmm exports ParserRegistry</bold>               <dim># Substring match "ParserRegistry"</dim>
  <dim>$</dim> <bold>fmm exports parser</bold>                       <dim># All exports containing "parser"</dim>
  <dim>$</dim> <bold>fmm exports --file crates/fmm-cli/src/cli/mod.rs</bold> <dim># All exports from one file</dim>
  <dim>$</dim> <bold>fmm exports '^Config'</bold>                    <dim># Regex: exports starting with "Config"</dim>
  <dim>$</dim> <bold>fmm exports 'Parser$'</bold>                    <dim># Regex: exports ending in "Parser"</dim>
  <dim>$</dim> <bold>fmm exports '^[A-Z]'</bold>                     <dim># Regex: PascalCase exports only</dim>
  <dim>$</dim> <bold>fmm exports Parser --dir crates/fmm-core/src/parser</bold> <dim># Scoped to directory</dim>
  <dim>$</dim> <bold>fmm exports Parser --limit 50 --offset 50</bold> <dim># Pagination</dim>
  <dim>$</dim> <bold>fmm exports Parser --json</bold>                 <dim># JSON output</dim>"#),
    )]
    Exports(ExportsCommandArgs),

    /// Start MCP server — 10 tools for LLM code navigation
    #[command(
        long_about = "Start the Model Context Protocol (MCP) server over stdio.\n\n\
            Exposes 10 tools that LLM agents (Claude, GPT, etc.) can call for O(1) \
            symbol lookup, dependency graphs, and surgical source reads — all without \
            reading entire files.",
        after_long_help = cstr!(
            r#"<bold><underline>Tools</underline></bold>
  <bold>fmm_lookup_export</bold>    Find which file defines a symbol — O(1)
  <bold>fmm_read_symbol</bold>      Extract exact source; use ClassName.method for methods
  <bold>fmm_dependency_graph</bold>  local_deps, external packages, and downstream blast radius
  <bold>fmm_file_outline</bold>     Table of contents with signatures, visibility, and kind
  <bold>fmm_list_exports</bold>     Search exports by pattern (fuzzy)
  <bold>fmm_search</bold>           Multi-criteria AND queries with relevance scoring
  <bold>fmm_list_files</bold>       List all indexed files under a directory path
  <bold>fmm_glossary</bold>         Symbol-level blast radius — all definitions + who imports each

<bold><underline>Setup</underline></bold>
  <dim>Add to .claude/settings.json or settings.local.json:</dim>
  <dim>{ "mcpServers": { "fmm": { "command": "fmm", "args": ["mcp"] } } }</dim>

<bold><underline>Notes</underline></bold>
  Communicates over stdio using the MCP JSON-RPC protocol.
  Requires the index to be built first ('fmm generate')."#),
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
    Completions(CompletionsCommandArgs),
}
