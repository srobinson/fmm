# fmm - Frontmatter Matters

**Structured metadata sidecars for LLM code navigation. 88-97% fewer tokens.**

[![CI](https://github.com/srobinson/fmm/actions/workflows/ci.yml/badge.svg)](https://github.com/srobinson/fmm/actions/workflows/ci.yml)

## The Problem

LLMs waste most of their context window just *finding* code. Every grep, glob, and file read burns tokens before reasoning even starts.

| Operation         | Without fmm       | With fmm sidecars          | Savings     |
| ----------------- | ----------------- | -------------------------- | ----------- |
| Understand 1 file | ~500 tokens       | 15-60 tokens               | 88-97%      |
| Scan 100 files    | ~50,000 tokens    | ~1,500 tokens              | 97%         |
| Context window    | Wasted on parsing | Reserved for reasoning     | Compounding |

## How It Works

fmm generates a `.fmm` **sidecar file** alongside each source file. The sidecar contains structured metadata — exports, imports, dependencies, LOC — so LLMs can navigate your codebase without reading source.

```
src/
  auth/
    session.ts          # 234 lines of source code
    session.ts.fmm      # 7 lines of metadata
    middleware.ts
    middleware.ts.fmm
  api/
    routes.ts
    routes.ts.fmm
```

A sidecar looks like this:

```yaml
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis-client]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-30
```

LLMs read sidecars to understand structure, then open source files only when they need to edit.

## Quick Start

```bash
# Install from source (requires Rust)
cargo install --path .

# Initialize configuration
fmm init

# Generate sidecars for your codebase
fmm generate src/

# See what would change (dry run)
fmm generate --dry-run src/

# Regenerate all sidecars (force update)
fmm update src/

# Validate sidecars are current (for CI)
fmm validate src/

# Remove all sidecars
fmm clean src/

# Check project status
fmm status
```

## LLM Integration

### MCP Server (Recommended)

fmm includes a built-in MCP server. LLMs query structured metadata instead of reading files.

Add to your Claude Code MCP configuration:

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

Available MCP tools:

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

### Navigation Pattern

```
LLM task: "Fix the session validation bug"

1. fmm_lookup_export("validateSession")  →  src/auth/session.ts
2. fmm_dependency_graph("src/auth/session.ts")  →  depends on ./types, ./config
3. LLM reads only session.ts, types.ts, config.ts

Cost: ~700 tokens instead of 50,000 (scanning everything)
```

## Configuration

Create `.fmmrc.json` in your project root (or run `fmm init`):

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024
}
```

## Supported Languages

| Language   | Extensions                                   | Custom Fields                                                             |
| ---------- | -------------------------------------------- | ------------------------------------------------------------------------- |
| TypeScript | `.ts`, `.tsx`                                | -                                                                         |
| JavaScript | `.js`, `.jsx`                                | -                                                                         |
| Python     | `.py`                                        | `decorators`                                                              |
| Rust       | `.rs`                                        | `derives`, `unsafe_blocks`, `trait_impls`, `lifetimes`, `async_functions` |
| Go         | `.go`                                        | -                                                                         |
| Java       | `.java`                                      | `annotations`                                                             |
| C++        | `.cpp`, `.hpp`, `.cc`, `.hh`, `.cxx`, `.hxx` | `namespaces`                                                              |
| C#         | `.cs`                                        | `namespaces`, `attributes`                                                |
| Ruby       | `.rb`                                        | `mixins`                                                                  |

All languages extract: **exports**, **imports**, **dependencies**, **LOC**.

## The Economics

| Model             | Cost per 1M tokens | 100-file scan without fmm | With fmm sidecars |
| ----------------- | ------------------ | ------------------------- | ----------------- |
| Claude Opus 4.5   | $5.00 input        | $0.25                     | $0.008            |
| Claude Sonnet 4.5 | $3.00 input        | $0.15                     | $0.005            |
| GPT-4o            | $2.50 input        | $0.13                     | $0.004            |

At scale: a coding assistant scanning your codebase 100 times/day saves $6-25/day on a 100-file project.

## Performance

- **Speed:** ~1,500 files/second on Apple Silicon
- **Single file parse:** <1ms (TypeScript, Python, Rust)
- **Batch 1000 files:** ~670ms total
- **Parallel:** All CPU cores via rayon
- **Incremental:** Only updates changed files
- **Memory:** Constant (streams files)

Run benchmarks: `cargo bench`

## CI/CD Integration

### GitHub Actions

```yaml
- name: Validate fmm sidecars
  run: |
    cargo install --path .
    fmm validate src/
```

### Pre-Commit Hook

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: fmm-update
        name: Update fmm sidecars
        entry: fmm update
        language: system
        pass_filenames: true
```

## How It Works Internally

1. **Parse** — tree-sitter parses source into AST
2. **Extract** — identifies exports, imports, dependencies per file
3. **Generate** — writes `.fmm` sidecar alongside each source file
4. **Query** — MCP server or CLI reads sidecars on demand, builds in-memory index

## Roadmap

- [x] 9 language parsers (TS/JS, Python, Rust, Go, Java, C++, C#, Ruby)
- [x] CLI: generate, update, validate, clean, search, status
- [x] MCP server with 5 query tools
- [x] Parallel processing (rayon)
- [x] Incremental updates
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
