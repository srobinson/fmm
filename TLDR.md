# fmm in brief

fmm indexes codebases so AI agents can navigate by structure instead of reading files one at a time.

## The problem

An LLM starting a task in an unfamiliar repo has no map. It resorts to `ls`, `grep`, and reading files until it builds enough context. This is slow and expensive in tokens.

## What fmm does

fmm parses every source file with tree-sitter, extracts structural metadata (exports, imports, dependencies, line counts), and stores it in a single SQLite database (`.fmm.db`) at the project root. The index is incremental: only changed files are re-parsed.

Agents query this index through 8 MCP tools (or the CLI) to answer structural questions in O(1):

- What files exist and how big are they?
- Where is a symbol defined?
- What does this file export?
- What depends on this module? (blast radius)
- What does the dependency graph look like?

## How it works

```
Source files  ->  tree-sitter AST  ->  extract exports/imports  ->  .fmm.db
                                                                       |
                                                          CLI or MCP queries
```

## Three crates

- **fmm-core**: Parsers (18 languages), manifest, search, config
- **fmm-cli**: CLI binary and MCP server
- **fmm-store**: SQLite persistence

## Key commands

```bash
fmm init                # Set up config and MCP server registration
fmm generate            # Index the codebase into .fmm.db
fmm watch               # Re-index on file change
fmm validate            # CI check: is the index current?
fmm search              # Query exports, imports, deps, files
fmm glossary <symbol>   # Who defines and imports this symbol?
fmm mcp                 # Start MCP server (JSON-RPC over stdio)
```

## MCP tools

| Tool | What it answers |
|---|---|
| `fmm_list_files` | What files exist in this directory? How big are they? |
| `fmm_file_outline` | What does this file contain? (exports with line ranges) |
| `fmm_lookup_export` | Which file defines this symbol? |
| `fmm_read_symbol` | Show me the source for this specific symbol |
| `fmm_list_exports` | Search exports by pattern across the codebase |
| `fmm_dependency_graph` | What does this file import? What imports it? |
| `fmm_glossary` | Full blast radius: all definitions + all consumers |
| `fmm_search` | Structured queries across the entire index |

## Languages

TypeScript and JavaScript have the deepest support. Python and Rust are well covered. Go, Java, C, C++, C#, Ruby, PHP, Swift, Kotlin, Dart, Elixir, Lua, Scala, and Zig have parsers with varying maturity.

## Performance

~1,500 files/second on Apple Silicon. Parallel across all cores. Incremental updates keep re-indexing fast.

## Install

```bash
npx frontmatter-matters init
```

Or build from source with `just install`.
