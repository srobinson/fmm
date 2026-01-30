# fmm — Frontmatter Matters

> **88-97% token reduction for LLM code navigation.**
> Proven across 5 experiments, 48+ runs, real codebases up to 9,008 files.

[![CI](https://github.com/srobinson/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/srobinson/fmm/actions/workflows/ci.yml)

## The Problem

LLMs waste most of their context window just *finding* code. A simple "where is X defined?" triggers dozens of grep/glob/read cycles — burning tokens on file contents the model never reasons about.

**Measured on a 244-file TypeScript codebase (Exp13):**

| Task | Without fmm | With fmm | Token Reduction |
|------|-------------|----------|-----------------|
| Code review | 1,824 lines read | 65 lines read | **96.4%** |
| Refactor analysis | 2,800 lines read | 345 lines read | **87.7%** |
| Architecture exploration | 7,135 lines read | 180 lines read | **97.5%** |

Without fmm, the LLM's strategy is always: `Grep → Read entire file → Summarize`. It never checks metadata directories or sidecar files organically — **0 out of 12 sessions** discovered `.fmm/` without explicit instruction ([Exp14](research/exp14/FINDINGS.md)).

## The Solution

fmm generates a `.fmm` **sidecar file** alongside each source file. The sidecar contains structured metadata — exports, imports, dependencies, LOC — so LLMs navigate your codebase without reading source.

```
src/
  auth/
    session.ts          ← 234 lines of source
    session.ts.fmm      ← 7 lines of metadata
    middleware.ts
    middleware.ts.fmm
  api/
    routes.ts
    routes.ts.fmm
```

```yaml
# session.ts.fmm
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis-client]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-30
```

LLMs read sidecars first, then open source files only when they need to edit.

## Quick Start

```bash
cargo install --path .
fmm init        # creates .fmmrc.json, skill, MCP config
fmm generate    # creates .fmm sidecars for all source files
```

That's it. Your AI coding assistant now navigates via metadata instead of brute-force file reads.

## How It Works

```
                        ┌─────────────────────────────────────────────────────┐
                        │                    fmm pipeline                     │
                        │                                                     │
  Source Files          │   ┌──────────┐    ┌───────────┐    ┌────────────┐  │    LLM / MCP Client
  ─────────────────────►│   │  Parser   │───►│ Extractor │───►│  Sidecar   │  │◄──────────────────
  .ts .py .rs .go       │   │(tree-sit) │    │           │    │  Writer    │  │   fmm_lookup_export
  .java .cpp .cs .rb    │   └──────────┘    └───────────┘    └─────┬──────┘  │   fmm_file_info
                        │                                          │         │   fmm_dependency_graph
                        │                                    ┌─────▼──────┐  │   fmm_list_exports
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

## Evidence

Five experiments validate fmm's impact. All data is reproducible — see the [research/](research/) directory.

| Experiment | Finding | Data |
|------------|---------|------|
| **Exp13**: Token reduction | 88-97% fewer tokens on real navigation tasks | 244-file TS codebase, 4 task types |
| **Exp14**: Organic discovery | LLMs never find `.fmm/` without instruction | 0/12 sessions across 4 conditions |
| **Exp15**: Delivery mechanism | Skill + MCP = optimal integration | 48 runs, 30% fewer tool calls vs CLAUDE.md alone |
| **Exp16**: Cost isolation | MCP eliminates grep, structured queries replace brute-force | Docker-isolated A/B comparison |
| **Proof harness** | -36% tool calls, -53% source reads on architecture queries | Live A/B with Claude Sonnet |

### Case Study: claude-flow (#1044)

Real-world bug fix on a 9,008-file repository:

```
$ fmm init                     # 3 seconds, 2,221 files indexed
$ fmm_lookup_export("model")   # → 2 files (not 50+)
$ # 5-line fix, clean PR pushed
```

Without fmm: ~30-50 file reads to find the bug. With fmm: 2.

## Navigation Pattern

```
LLM task: "Fix the session validation bug"

1. fmm_lookup_export("validateSession")  →  src/auth/session.ts
2. fmm_dependency_graph("src/auth/session.ts")  →  depends on ./types, ./config
3. LLM reads only session.ts, types.ts, config.ts

Result: ~700 tokens instead of ~50,000 (scanning everything)
```

## LLM Integration

### MCP Server (Recommended)

fmm includes a built-in MCP server. Add to your Claude Code configuration:

```json
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["mcp"]
    }
  }
}
```

| Tool | Purpose |
|------|---------|
| `fmm_lookup_export` | Find which file exports a symbol (O(1) lookup) |
| `fmm_list_exports` | List exports matching a pattern or from a file |
| `fmm_file_info` | Get a file's structural profile |
| `fmm_dependency_graph` | Get upstream dependencies and downstream dependents |
| `fmm_search` | Search by export, imports, dependencies, LOC range |

### Search CLI

```bash
fmm search --export validateUser      # Find file by export
fmm search --imports crypto           # Files importing crypto
fmm search --loc ">500"               # Large files
fmm search --depends-on ./types       # Files depending on module
fmm search --json                     # Output as JSON
```

## The Economics

| Model | Cost/1M tokens | 100-file scan without fmm | With fmm sidecars | Savings |
|-------|----------------|---------------------------|--------------------| --------|
| Claude Opus 4.5 | $5.00 | $0.25 | $0.008 | **97%** |
| Claude Sonnet 4.5 | $3.00 | $0.15 | $0.005 | **97%** |
| GPT-4o | $2.50 | $0.13 | $0.004 | **97%** |

**At scale** ([Exp15](research/exp15/FINDINGS.md)):
- Solo developer (50 queries/day): **$6K-10K/year saved**
- Small team (500 queries/day): **$10K-25K/year saved**
- Enterprise (10K queries/day): **$50K+/year saved**

## Configuration

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024
}
```

Create with `fmm init` or manually as `.fmmrc.json`.

## Supported Languages

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

Extensible via [C FFI plugin system](docs/plugin-architecture.md).

## Performance

- **~1,500 files/second** on Apple Silicon
- **<1ms** per file parse (TypeScript, Python, Rust)
- **Parallel** across all CPU cores (rayon)
- **Incremental** — only updates changed files
- **Constant memory** — streams files

## CI/CD Integration

```yaml
# GitHub Actions
- name: Validate fmm sidecars
  run: |
    cargo install --path .
    fmm validate src/
```

```yaml
# Pre-commit hook
repos:
  - repo: local
    hooks:
      - id: fmm-update
        name: Update fmm sidecars
        entry: fmm update
        language: system
        pass_filenames: true
```

## CLI Reference

| Command | Purpose |
|---------|---------|
| `fmm init` | Initialize config, skill, and MCP server setup |
| `fmm generate [path]` | Create sidecars for files that don't have them |
| `fmm update [path]` | Regenerate all sidecars |
| `fmm validate [path]` | Check sidecars are current (CI-friendly) |
| `fmm clean [path]` | Remove all sidecar files |
| `fmm status` | Show project status and configuration |
| `fmm search` | Query sidecars by export, import, LOC, dependency |
| `fmm mcp` | Start MCP server |

## Roadmap

- [x] 9 language parsers (TS/JS, Python, Rust, Go, Java, C++, C#, Ruby)
- [x] CLI: generate, update, validate, clean, search, status
- [x] MCP server with 5 query tools
- [x] Parallel processing (rayon)
- [x] Incremental updates
- [x] Plugin architecture (C FFI)
- [ ] Watch mode (auto-update on save)
- [ ] Complexity metrics
- [ ] VS Code extension

## Contributing

PRs welcome. Especially:

- New language parsers
- LLM integration examples
- Token reduction benchmarks

## License

MIT

---

Built by Stuart Robinson ([@srobinson](https://github.com/srobinson)) with research assistance from Claude.
