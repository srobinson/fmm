# fmm — Frontmatter Matters

**Structural intelligence for codebases.**

[![CI](https://github.com/srobinson/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/srobinson/fmm/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

```bash
npx frontmatter-matters init
```

|                          | Native tooling                              | fmm                                              |
| ------------------------ | ------------------------------------------- | ------------------------------------------------ |
| **Best at**              | Local truth: text search, call sites, variable definitions, exact in-file matches | Codebase shape: topology, exports, outlines, dependency graphs, blast radius |
| **First question**       | "What is in this repo?" answered by `ls`, `grep`, and file reads | Query indexed structure before opening source |
| **What you learn first** | Individual files, one at a time             | Directory shape, largest files, exports, and high-risk files |
| **Reading pattern**      | Search text, then inspect files             | Start with structure, then open only the files that matter |
| **Result**               | Precise local inspection                    | Faster orientation and targeted navigation       |

## What it does

fmm maintains a single SQLite database (`.fmm.db`) at the project root. Each indexed file contributes its exports, imports, dependencies, and line count. The database supports incremental updates — only changed files are re-parsed.

```
src/auth/session.ts   ← 234 lines of source
.fmm.db               ← single index for the entire project
```

fmm turns a repo into queryable structure: file topology, exports, imports, dependencies, LOC, and file outlines with line ranges.

The main value is faster orientation. Agents can see what the codebase contains, where the intellectual weight lives, and which files have the biggest blast radius before they start opening source.

fmm is not a replacement for `grep`, editor search, or reading source files. Native tools are still better for intra-file call sites, variable definitions, and exact text matches. fmm is the index you query first when the question is about codebase shape.

LLMs query the index first via MCP tools, then open source files when they need source-level detail.

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

That's it. Your AI coding assistant can now start with structure instead of blind file-by-file exploration.

Use native tooling alongside fmm:

- Use `fmm` for orientation, symbol lookup, dependency analysis, outlines, and blast radius.
- Use `rg`, editor search, or file reads for local pattern search, non-indexed variables, and intra-file call sites.

## Commands

| Command               | Purpose                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `fmm init`            | Set up config, Claude skill, and MCP server                        |
| `fmm generate [path]` | Index source files into `.fmm.db` (exports, imports, deps, LOC)   |
| `fmm watch [path]`    | Watch source files and update the index on change                  |
| `fmm validate [path]` | Check the index is current (CI-friendly, exit 1 if stale)          |
| `fmm search`          | Query indexed structure: exports, imports, dependencies, LOC, and file-level matches |
| `fmm glossary`        | Symbol-level impact analysis — who imports this export?            |
| `fmm mcp`             | Start MCP server (8 tools for LLM navigation)                      |
| `fmm status`          | Show config and workspace stats                                    |
| `fmm clean [path]`    | Clear the fmm index database                                       |

Run `fmm --help` for workflows and examples, or `fmm <command> --help` for detailed per-command help.

## MCP Tools

fmm includes a built-in MCP server with 8 tools. Configure via `fmm init --mcp` or manually:

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
| `fmm_search`           | Indexed structural queries across exports, files, imports, and dependencies   |
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

## When to Use It

- Use `fmm` when you need repo-wide structure: "What lives here?", "Which files define this API?", "What depends on this module?", "Where is the blast radius?"
- Use native tools when you need local truth: "Where is this variable assigned?", "What calls this helper inside the same file?", "Show every textual match in checker.ts."
- Use both together: start with `fmm` to narrow the search space, then switch to `rg`, editor search, or direct file reads for detailed inspection.

## Language Coverage

TypeScript is the most mature and fully tested language in fmm today. Python and Rust have meaningful coverage but are less battle-tested. The remaining language parsers exist and may be useful, but they have not yet had the same level of validation.

If you rely on one of the less-tested languages, contributions are welcome: parser fixes, edge-case fixtures, validation passes, and real-world case studies all help tighten support.

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

Across languages, fmm aims to extract: **exports**, **imports**, **dependencies**, and **LOC**. The depth and reliability of extraction currently varies by language maturity.

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

The practical win is better navigation: quicker orientation and more precise follow-up reads.

## Contributing

PRs welcome. Especially:

- New language parsers
- LLM integration examples
- Large-repo workflows and case studies

## License

MIT

---

Built by Stuart Robinson ([@srobinson](https://github.com/srobinson)) with research assistance from Claude.
