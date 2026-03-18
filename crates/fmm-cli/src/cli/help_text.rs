use color_print::cstr;

pub const LONG_ABOUT: &str = "\
Frontmatter Matters — 80-90% fewer file reads for LLM agents";

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
  <bold>mcp</bold>           Start MCP server (9 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Clear the fmm index database

Use <bold>--help</bold> for workflows and examples.
https://github.com/srobinson/fmm"#
);

// Full help (--help): commands + MCP tools + workflows + languages
pub const LONG_HELP: &str = cstr!(
    r#"<bold><underline>Navigation Commands</underline></bold>
  <bold>lookup</bold> SYMBOL       Find where a symbol is defined — O(1)
  <bold>read</bold> SYMBOL         Extract exact source for a symbol or ClassName.method
  <bold>deps</bold> FILE           Dependency graph: local_deps, external, downstream
  <bold>outline</bold> FILE        File table-of-contents with line ranges
  <bold>ls</bold> [DIR]           List indexed files under a directory
  <bold>exports</bold> [PATTERN]  Search exports by pattern (substring or regex, auto-detected)
  <bold>search</bold>             Smart search — exports, files, imports (just works)
  <bold>glossary</bold>           Symbol-level impact analysis — who uses this export?

<bold><underline>Project Commands</underline></bold>
  <bold>init</bold>          Set up config, Claude skill, and MCP server
  <bold>generate</bold>      Index source files into the SQLite database (exports, imports, deps, LOC)
  <bold>watch</bold>         Watch source files and update the index on change
  <bold>validate</bold>      Check the index is current (CI-friendly, exit 1 if stale)
  <bold>mcp</bold>           Start MCP server (9 tools for LLM navigation)
  <bold>status</bold>        Show config and workspace stats
  <bold>clean</bold>         Clear the fmm index database

<bold><underline>Navigation Examples</underline></bold>
  <dim>$</dim> <bold>fmm lookup Injector</bold>                         <dim># File + line range + deps</dim>
  <dim>$</dim> <bold>fmm read Injector</bold>                           <dim># Full class source</dim>
  <dim>$</dim> <bold>fmm read Injector.loadInstance</bold>              <dim># Single method source</dim>
  <dim>$</dim> <bold>fmm read Injector --no-truncate</bold>            <dim># Bypass 10KB cap</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts</bold>                    <dim># Direct dependency graph</dim>
  <dim>$</dim> <bold>fmm deps src/injector.ts --depth 2</bold>         <dim># Transitive (2 hops)</dim>
  <dim>$</dim> <bold>fmm outline src/injector.ts</bold>                 <dim># Exports with line ranges</dim>
  <dim>$</dim> <bold>fmm ls src/</bold>                                 <dim># Files in src/</dim>
  <dim>$</dim> <bold>fmm ls --sort-by downstream</bold>                  <dim># Most-imported files first (pre-refactoring)</dim>
  <dim>$</dim> <bold>fmm exports Module</bold>                          <dim># All exports matching "Module"</dim>
  <dim>$</dim> <bold>fmm exports Module --dir packages/core/</bold>     <dim># Scoped to directory</dim>
  <dim>$</dim> <bold>fmm lookup Injector --json | jq .file</bold>      <dim># Machine-readable output</dim>

<bold><underline>Project Workflows</underline></bold>
  <dim>$</dim> <bold>fmm init</bold>                              <dim># One-command setup</dim>
  <dim>$</dim> <bold>fmm generate && fmm validate</bold>          <dim># CI pipeline</dim>
  <dim>$</dim> <bold>fmm search store</bold>                      <dim># Smart search across everything</dim>
  <dim>$</dim> <bold>fmm glossary config</bold>                   <dim># Who uses Config, loadConfig, AppConfig?</dim>

<bold><underline>Languages</underline></bold>
  TypeScript · JavaScript · Python · Rust · Go · Java · C++ · C# · Ruby

88-97% token reduction measured on real codebases.
https://github.com/srobinson/fmm"#
);

// Custom help template — our before_help already lists commands,
// so we skip the auto-generated {subcommands} section
pub const HELP_TEMPLATE: &str = "{about-with-newline}\n{before-help}\n";
