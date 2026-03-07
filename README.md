# fmm — Frontmatter Matters

**80-90% fewer file reads for LLM agents.**

[![CI](https://github.com/srobinson/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/srobinson/fmm/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Languages](https://img.shields.io/badge/languages-17-informational.svg)](#supported-languages)

```bash
npx frontmatter-matters init
```

|                          | Without fmm                                 | With fmm                                         |
| ------------------------ | ------------------------------------------- | ------------------------------------------------ |
| **How LLM navigates**    | grep → read entire file → summarize, repeat | Query SQLite index → open only needed files      |
| **Tokens for 500 files** | ~50,000                                     | ~2,000                                           |
| **Reduction**            | —                                           | **88-97%**                                       |

## What it does

fmm maintains a single SQLite database (`.fmm.db`) at the project root. Each indexed file contributes its exports, imports, dependencies, and line count. The database supports incremental updates — only changed files are re-parsed.

```
src/auth/session.ts   ← 234 lines of source
.fmm.db               ← single index for the entire project
```

LLMs query the index first via MCP tools, then open source files only when they need to edit.

## Installation

```bash
# npm (recommended)
npx frontmatter-matters --help

# Or install globally
npm install -g frontmatter-matters
```

## Quick start

```bash
fmm init                              # One-command setup
fmm generate && fmm validate          # CI pipeline
fmm watch                             # Live index updates on file change
fmm search --export createStore       # O(1) symbol lookup
fmm search --depends-on src/auth.ts   # Impact analysis
fmm search --loc ">500"              # Find large files
fmm search --imports react --json     # Structured output
```

That's it. Your AI coding assistant now navigates via metadata instead of brute-force file reads.

## Commands

| Command               | Purpose                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `fmm init`            | Set up config, Claude skill, and MCP server                        |
| `fmm generate [path]` | Index source files into `.fmm.db` (exports, imports, deps, LOC)   |
| `fmm watch [path]`    | Watch source files and update the index on change                  |
| `fmm validate [path]` | Check the index is current (CI-friendly, exit 1 if stale)          |
| `fmm search`          | Query the index (O(1) export lookup, dependency graphs)            |
| `fmm glossary`        | Symbol-level impact analysis — who imports this export?            |
| `fmm mcp`             | Start MCP server (9 tools for LLM navigation)                      |
| `fmm status`          | Show config and workspace stats                                    |
| `fmm clean [path]`    | Clear the fmm index database                                       |

Run `fmm --help` for workflows and examples, or `fmm <command> --help` for detailed per-command help.

## MCP Tools

fmm includes a built-in MCP server with 9 tools. Configure via `fmm init --mcp` or manually:

```json
{
  "mcpServers": {
    "fmm": {
      "command": "npx",
      "args": ["frontmatter-matters", "mcp"]
    }
  }
}
```

| Tool                   | Purpose                                                                       |
| ---------------------- | ----------------------------------------------------------------------------- |
| `fmm_lookup_export`    | Find which file defines a symbol — O(1)                                       |
| `fmm_read_symbol`      | Extract exact source; `ClassName.method` for public or private methods; `line_numbers: true` to annotate lines; follows re-export chains |
| `fmm_dependency_graph` | Intra-project deps (`local_deps`), external packages, and downstream blast radius. `filter: "source"` excludes test files; `filter: "tests"` shows test coverage |
| `fmm_file_outline`     | Table of contents with line ranges; `include_private: true` shows private/protected members |
| `fmm_list_exports`     | Search exports by pattern — substring (case-insensitive) or regex (auto-detected: `^handle`, `Service$`, `^[A-Z]`) |
| `fmm_search`           | Multi-criteria AND queries with relevance scoring                             |
| `fmm_list_files`       | List all indexed files under a directory path                                 |
| `fmm_glossary`         | Symbol-level blast radius — all definitions of X + files that import each one |

## How it works

```
                        ┌──────────────────────────────────────────────────────┐
                        │                     fmm pipeline                     │
                        │                                                      │
  Source Files          │   ┌───────────┐    ┌───────────┐    ┌────────────┐   │    LLM / MCP Client
  ─────────────────────►│   │  Parser   │───►│ Extractor │───►│  SQLite    │   │◄──────────────────
  .ts .py .rs .go .c    │   │(tree-sit) │    │           │    │  Writer    │   │   fmm_lookup_export
  .java .cpp .cs .rb    │   └───────────┘    └───────────┘    └─────┬──────┘   │   fmm_read_symbol
  .dart .lua .zig .sc   │                                     ┌─────▼──────┐   │   fmm_dependency_graph
                        │                                     │  .fmm.db   │   │   fmm_file_outline
                        │                                     │  (SQLite)  │   │   fmm_list_exports
                        │                                     └─────┬──────┘   │   fmm_search
                        │                                           │          │   fmm_list_files
                        │                                           │          │   fmm_glossary
                        │                                     ┌─────▼──────┐   │
                        │                                     │  In-memory │   │
                        │                                     │   Index    │───┼──► Query Results
                        │                                     └────────────┘   │
                        └──────────────────────────────────────────────────────┘
```

1. **Parse** — tree-sitter parses source into AST
2. **Extract** — identifies exports, imports, dependencies per file
3. **Generate** — upserts file data into `.fmm.db` (incremental, mtime-based)
4. **Query** — MCP server or CLI loads the index from SQLite in milliseconds

## Supported Languages

TypeScript · JavaScript · Python · Rust · Go · Java · C · C++ · C# · Ruby · PHP · Swift · Kotlin · Dart · Elixir · Lua · Scala · Zig

| Language   | Extensions                                   | Custom Fields                                                             |
| ---------- | -------------------------------------------- | ------------------------------------------------------------------------- |
| TypeScript | `.ts`, `.tsx`                                | —                                                                         |
| JavaScript | `.js`, `.jsx`                                | —                                                                         |
| Python     | `.py`                                        | `decorators`                                                              |
| Rust       | `.rs`                                        | `derives`, `unsafe_blocks`, `trait_impls`, `lifetimes`, `async_functions` |
| Go         | `.go`                                        | —                                                                         |
| Java       | `.java`                                      | `annotations`                                                             |
| C          | `.c`, `.h`                                   | `macros`, `typedefs`                                                      |
| C++        | `.cpp`, `.hpp`, `.cc`, `.hh`, `.cxx`, `.hxx` | `namespaces`                                                              |
| C#         | `.cs`                                        | `namespaces`, `attributes`                                                |
| Ruby       | `.rb`                                        | `mixins`                                                                  |
| PHP        | `.php`                                       | `namespaces`, `traits_used`                                               |
| Swift      | `.swift`                                     | `protocols`, `extensions`                                                 |
| Kotlin     | `.kt`, `.kts`                                | `data_classes`, `sealed_classes`, `companion_objects`                     |
| Dart       | `.dart`                                      | `mixins`, `extensions`                                                    |
| Elixir     | `.ex`, `.exs`                                | `macros`, `protocols`, `behaviours`                                       |
| Lua        | `.lua`                                       | —                                                                         |
| Scala      | `.scala`, `.sc`                              | `case_classes`, `implicits`, `annotations`                                |
| Zig        | `.zig`                                       | `comptime_blocks`, `test_blocks`                                          |

All languages extract: **exports**, **imports**, **dependencies**, **LOC**.

## Performance

- **~1,500 files/second** on Apple Silicon
- **<1ms** per file parse (TypeScript, Python, Rust)
- **Parallel** across all CPU cores (rayon)
- **Incremental** — only updates changed files

## CI/CD Integration

```yaml
# GitHub Actions
- name: Validate fmm index
  run: |
    npx frontmatter-matters validate
```

88-97% token reduction measured on real codebases.

## Contributing

PRs welcome. Especially:

- New language parsers
- LLM integration examples
- Token reduction benchmarks

## License

MIT

---

Built by Stuart Robinson ([@srobinson](https://github.com/srobinson)) with research assistance from Claude.
