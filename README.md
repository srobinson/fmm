# fmm — Frontmatter Matters

**Metadata sidecars that give LLMs a map of your codebase.**

[![CI](https://github.com/mdcontext/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/mdcontext/fmm/actions/workflows/ci.yml)
[![Docs](https://github.com/mdcontext/fmm/actions/workflows/docs.yml/badge.svg)](https://mdcontext.github.io/fmm/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Languages](https://img.shields.io/badge/languages-9-informational.svg)](#supported-languages)

<!-- TODO: Replace with animated SVG from asciinema recording
     Record: asciinema rec --command "./demos/01-getting-started.sh"
     Convert: svg-term --in demos/01-getting-started.cast --out docs/src/hero.svg -->

```bash
cargo install fmm && cd your-project && fmm init
```

|  | Without fmm | With fmm |
|--|------------|----------|
| **How LLM navigates** | grep → read entire file → summarize, repeat | Read sidecar metadata → open only needed files |
| **Tokens for 500 files** | ~50,000 | ~2,000 |
| **Reduction** | — | **96%** |

## What it does

fmm generates a `.fmm` sidecar file alongside each source file. The sidecar is a tiny YAML file listing exports, imports, dependencies, and line count:

```
src/auth/session.ts      ← 234 lines of source
src/auth/session.ts.fmm  ← 7 lines of metadata
```

```yaml
---
file: src/auth/session.ts
exports: [createSession, validateSession, destroySession]
imports: [jsonwebtoken]
dependencies: [../config, ../db/users]
loc: 234
---
```

LLMs read sidecars first, then open source files only when they need to edit.

## Quick start

```bash
cargo install fmm
cd your-project
fmm init        # creates config, skill, MCP server + generates sidecars
```

That's it. Your AI coding assistant now navigates via metadata instead of brute-force file reads.

Try a search:

```bash
fmm search --export createStore     # O(1) symbol lookup
fmm search --imports react          # find all React consumers
fmm search --loc ">500"             # find large files
fmm search --depends-on src/db.ts   # impact analysis
```

See the [demo project](examples/demo-project/) for a hands-on walkthrough.

## How it works

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

## LLM Integration

### MCP Server (Recommended)

fmm includes a built-in MCP server. Configure via `fmm init --mcp` or manually:

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

See the full [MCP Tools reference](https://mdcontext.github.io/fmm/reference/mcp-tools.html) for schemas and examples.

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

## Performance

- **~1,500 files/second** on Apple Silicon
- **<1ms** per file parse (TypeScript, Python, Rust)
- **Parallel** across all CPU cores (rayon)
- **Incremental** — only updates changed files
- **Constant memory** — streams files

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
| `fmm completions <shell>` | Generate shell completions (bash, zsh, fish, powershell) |

Full reference: [CLI docs](https://mdcontext.github.io/fmm/reference/cli.html)

## CI/CD Integration

```yaml
# GitHub Actions
- name: Validate fmm sidecars
  run: |
    cargo install fmm
    fmm validate src/
```

## Documentation

- [Getting Started](https://mdcontext.github.io/fmm/getting-started/quickstart.html) — first sidecar in 60 seconds
- [CLI Reference](https://mdcontext.github.io/fmm/reference/cli.html) — all commands and options
- [Sidecar Format](https://mdcontext.github.io/fmm/reference/sidecar-format.html) — YAML specification
- [MCP Tools](https://mdcontext.github.io/fmm/reference/mcp-tools.html) — tool schemas for LLM agents
- [Configuration](https://mdcontext.github.io/fmm/reference/configuration.html) — .fmmrc.json options
- [llms.txt](https://mdcontext.github.io/fmm/llms.txt) — AI-readable documentation index

## Contributing

PRs welcome. Especially:

- New language parsers
- LLM integration examples
- Token reduction benchmarks

## License

MIT

---

Built by Stuart Robinson ([@srobinson](https://github.com/srobinson)) with research assistance from Claude.
