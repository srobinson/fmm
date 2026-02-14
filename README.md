# fmm — Frontmatter Matters

**80-90% fewer file reads for LLM agents.**

[![CI](https://github.com/srobinson/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/srobinson/fmm/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Languages](https://img.shields.io/badge/languages-9-informational.svg)](#supported-languages)

```bash
npx frontmatter-matters init
```

|  | Without fmm | With fmm |
|--|------------|----------|
| **How LLM navigates** | grep → read entire file → summarize, repeat | Read sidecar metadata → open only needed files |
| **Tokens for 500 files** | ~50,000 | ~2,000 |
| **Reduction** | — | **88-97%** |

## What it does

fmm generates a `.fmm` sidecar file alongside each source file. The sidecar is a tiny YAML file listing exports, imports, dependencies, and line count:

```
src/auth/session.ts      ← 234 lines of source
src/auth/session.ts.fmm  ← 7 lines of metadata
```

```yaml
---
file: src/auth/session.ts
exports:
  createSession: [12, 45]
  validateSession: [47, 89]
  destroySession: [91, 110]
imports: [jsonwebtoken]
dependencies: [../config, ../db/users]
loc: 234
---
```

LLMs read sidecars first, then open source files only when they need to edit.

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
fmm watch                             # Live sidecar regeneration
fmm search --export createStore       # O(1) symbol lookup
fmm search --depends-on src/auth.ts   # Impact analysis
fmm search --loc ">500"              # Find large files
fmm search --imports react --json     # Structured output
```

That's it. Your AI coding assistant now navigates via metadata instead of brute-force file reads.

## Commands

| Command | Purpose |
|---------|---------|
| `fmm init` | Set up config, Claude skill, and MCP server |
| `fmm generate [path]` | Create and update .fmm sidecars (exports, imports, deps, LOC) |
| `fmm watch [path]` | Watch source files and regenerate sidecars on change |
| `fmm validate [path]` | Check sidecars are current (CI-friendly, exit 1 if stale) |
| `fmm search` | Query the index (O(1) export lookup, dependency graphs) |
| `fmm mcp` | Start MCP server (7 tools for LLM navigation) |
| `fmm status` | Show config and workspace stats |
| `fmm clean [path]` | Remove all .fmm sidecars |

Run `fmm --help` for workflows and examples, or `fmm <command> --help` for detailed per-command help.

## MCP Tools

fmm includes a built-in MCP server with 7 tools. Configure via `fmm init --mcp` or manually:

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

| Tool | Purpose |
|------|---------|
| `fmm_lookup_export` | Find which file defines a symbol — O(1) |
| `fmm_read_symbol` | Extract exact source by symbol name (line ranges) |
| `fmm_dependency_graph` | Upstream deps + downstream dependents |
| `fmm_file_outline` | Table of contents with line ranges |
| `fmm_list_exports` | Search exports by pattern (fuzzy) |
| `fmm_file_info` | Structural profile without reading source |
| `fmm_search` | Multi-criteria AND queries |

## How it works

```
                        ┌─────────────────────────────────────────────────────┐
                        │                    fmm pipeline                     │
                        │                                                     │
  Source Files          │   ┌──────────┐    ┌───────────┐    ┌────────────┐  │    LLM / MCP Client
  ─────────────────────►│   │  Parser   │───►│ Extractor │───►│  Sidecar   │  │◄──────────────────
  .ts .py .rs .go       │   │(tree-sit) │    │           │    │  Writer    │  │   fmm_lookup_export
  .java .cpp .cs .rb    │   └──────────┘    └───────────┘    └─────┬──────┘  │   fmm_read_symbol
                        │                                          │         │   fmm_dependency_graph
                        │                                    ┌─────▼──────┐  │   fmm_file_outline
                        │                                    │  .fmm      │  │   fmm_search
                        │                                    │  sidecars   │  │
                        │                                    └─────┬──────┘  │
                        │                                          │         │
                        │                                    ┌─────▼──────┐  │
                        │                                    │ In-memory  │  │
                        │                                    │   Index    │──┼──► Query Results
                        │                                    └────────────┘  │
                        └─────────────────────────────────────────────────────┘
```

1. **Parse** — tree-sitter parses source into AST
2. **Extract** — identifies exports, imports, dependencies per file
3. **Generate** — writes `.fmm` sidecar alongside each source file
4. **Query** — MCP server or CLI reads sidecars on demand, builds in-memory index

## Supported Languages

TypeScript · JavaScript · Python · Rust · Go · Java · C++ · C# · Ruby

| Language | Extensions | Custom Fields |
|----------|-----------|---------------|
| TypeScript | `.ts`, `.tsx` | — |
| JavaScript | `.js`, `.jsx` | — |
| Python | `.py` | `decorators` |
| Rust | `.rs` | `derives`, `unsafe_blocks`, `trait_impls`, `lifetimes`, `async_functions` |
| Go | `.go` | — |
| Java | `.java` | `annotations` |
| C++ | `.cpp`, `.hpp`, `.cc`, `.hh`, `.cxx`, `.hxx` | `namespaces` |
| C# | `.cs` | `namespaces`, `attributes` |
| Ruby | `.rb` | `mixins` |

All languages extract: **exports**, **imports**, **dependencies**, **LOC**.

## Performance

- **~1,500 files/second** on Apple Silicon
- **<1ms** per file parse (TypeScript, Python, Rust)
- **Parallel** across all CPU cores (rayon)
- **Incremental** — only updates changed files

## CI/CD Integration

```yaml
# GitHub Actions
- name: Validate fmm sidecars
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
