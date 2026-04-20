use color_print::cstr;

pub const LONG_ABOUT: &str = "\
Frontmatter Matters: Structural intelligence for codebases";

// Short help (-h): commands + hint to use --help
pub const SHORT_HELP: &str = cstr!(
    r#"<bold><underline>Navigation</underline></bold>
  <bold>lookup</bold>        Find where a symbol is defined — O(1)
  <bold>read</bold>          Extract exact source for a symbol or method
  <bold>deps</bold>          Dependency graph: local_deps, external, downstream
  <bold>outline</bold>       File table-of-contents with line ranges
  <bold>ls</bold>            List indexed files under a directory
  <bold>exports</bold>       Search exports by pattern
  <bold>search</bold>        Smart search — exports, files, imports (just works)
  <bold>glossary</bold>      Symbol-level impact analysis — who uses this export?

<bold><underline>Project</underline></bold>
  <bold>init</bold>          Set up config, Claude skill, and MCP server
  <bold>generate</bold>      Index source files into the SQLite database (exports, imports, deps, LOC)
  <bold>watch</bold>         Watch source files and update the index on change
  <bold>validate</bold>      Check the index is current (CI-friendly, exit 1 if stale)
  <bold>mcp</bold>           Start MCP server (8 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Clear the fmm index database

Use <bold>--help</bold> for workflows and examples.
https://github.com/srobinson/fmm"#
);

// Full help (--help): commands + MCP tools + workflows + languages
pub const LONG_HELP: &str = cstr!(
    r#"<bold><underline>Navigation Commands</underline></bold>
  <bold>ls</bold> [DIR]           Indexed files with sorting, grouping, filters, and pagination
  <bold>outline</bold> FILE        File table of contents with exports and method ranges
  <bold>lookup</bold> SYMBOL       O(1) definition lookup with file metadata
  <bold>exports</bold> [PATTERN]  Export discovery by substring, regex, file, or directory
  <bold>read</bold> SYMBOL         Exact source for Symbol or ClassName.method
  <bold>deps</bold> FILE           Imports, downstream dependents, and transitive blast radius
  <bold>search</bold> [TERM]       Smart search across exports, files, imports, and LOC filters
  <bold>glossary</bold> PATTERN    Impact analysis: definitions, importers, and call sites

<bold><underline>Project Commands</underline></bold>
  <bold>init</bold>          Set up config, Claude skill, and MCP server
  <bold>generate</bold>      Index source files into the SQLite database (exports, imports, deps, LOC)
  <bold>watch</bold>         Watch source files and update the index on change
  <bold>validate</bold>      Check the index is current (CI-friendly, exit 1 if stale)
  <bold>mcp</bold>           Start MCP server (8 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Clear the fmm index database

<bold><underline>Core Workflow</underline></bold>
  <dim>$</dim> <bold>fmm ls crates/fmm-core/src --sort-by downstream --limit 20</bold> <dim># Start with high blast-radius files</dim>
  <dim>$</dim> <bold>fmm outline crates/fmm-core/src/parser/mod.rs --include-private</bold> <dim># Inspect structure before reading source</dim>
  <dim>$</dim> <bold>fmm lookup ParserRegistry</bold>                  <dim># Find the definition and file profile</dim>
  <dim>$</dim> <bold>fmm read ParserRegistry.get_parser --line-numbers</bold> <dim># Read one method with exact lines</dim>
  <dim>$</dim> <bold>fmm deps crates/fmm-core/src/parser/mod.rs --depth 2 --filter source</bold> <dim># Production impact radius</dim>

<bold><underline>Discovery</underline></bold>
  <dim>$</dim> <bold>fmm exports ParserRegistry</bold>                    <dim># Fuzzy export search</dim>
  <dim>$</dim> <bold>fmm exports '^Config' --dir crates/fmm-core/src</bold> <dim># Regex search scoped to a directory</dim>
  <dim>$</dim> <bold>fmm exports --file crates/fmm-cli/src/cli/mod.rs</bold> <dim># Exports from one file</dim>
  <dim>$</dim> <bold>fmm search parser</bold>                             <dim># Smart search across indexed metadata</dim>
  <dim>$</dim> <bold>fmm search --imports anyhow --min-loc 100</bold>     <dim># Files importing anyhow with size filter</dim>
  <dim>$</dim> <bold>fmm search parser --limit 5</bold>                   <dim># Cap fuzzy export results</dim>

<bold><underline>Impact and Output</underline></bold>
  <dim>$</dim> <bold>fmm glossary ParserRegistry</bold>                   <dim># Files importing the defining module</dim>
  <dim>$</dim> <bold>fmm glossary ParserRegistry.get_parser --precision call-site</bold> <dim># Confirm direct callers</dim>
  <dim>$</dim> <bold>fmm glossary Config --mode tests --no-truncate</bold> <dim># Test coverage impact without output cap</dim>
  <dim>$</dim> <bold>fmm read Commands --no-truncate</bold>               <dim># Bypass the 10KB source cap</dim>
  <dim>$</dim> <bold>fmm lookup Cli --json | jq .file</bold>              <dim># Machine-readable output</dim>

<bold><underline>Project Workflows</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                                      <dim># One-command setup</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>                  <dim># CI-friendly index refresh</dim>
  <dim>$</dim> <bold>fmm status</bold>                                    <dim># Config, languages, and index counts</dim>
  <dim>$</dim> <bold>fmm watch</bold>                                     <dim># Keep the index fresh during development</dim>
  <dim>$</dim> <bold>fmm mcp</bold>                                       <dim># Start the MCP server over stdio</dim>
  <dim>$</dim> <bold>fmm clean</bold>                                     <dim># Clear the local index database</dim>

Use <bold>fmm <<command>> --help</bold> for every flag and command-specific examples.

<bold><underline>Languages</underline></bold>
  TypeScript · JavaScript · Python · Rust

Built to replace broad file reads with indexed, structural navigation.
https://github.com/srobinson/fmm"#
);

// Custom help template — our before_help already lists commands,
// so we skip the auto-generated {subcommands} section
pub const HELP_TEMPLATE: &str = "{about-with-newline}\n{before-help}\n";
