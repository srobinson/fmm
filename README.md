# fmm - Frontmatter Matters

**Infrastructure for LLM cost reduction. 88-97% fewer tokens for code understanding.**

## The Problem

LLMs are the developers now. Every time an LLM reads your codebase, you pay for it:

| Operation | Without fmm | With fmm manifest | Savings |
|-----------|-------------|-------------------|---------|
| Understand 1 file | ~500 tokens | 15-60 tokens | 88-97% |
| Scan 100 files | ~50,000 tokens | ~1,500 tokens | 97% |
| Context window | Wasted on parsing | Reserved for reasoning | Compounding |

**fmm generates structured metadata that LLMs can query instead of read.**

## How It Works

### 1. Manifest JSON (Primary Output)

The real value is the manifest - a single JSON file LLMs can query:

```json
{
  "version": "1.0.0",
  "generated_at": "2026-01-28T12:00:00Z",
  "files": [
    {
      "file": "src/auth/session.ts",
      "exports": ["createSession", "validateSession", "destroySession"],
      "imports": ["jwt", "redis-client"],
      "dependencies": ["./types", "./config"],
      "loc": 234,
      "modified": "2026-01-27"
    },
    {
      "file": "src/api/routes.ts",
      "exports": ["router", "authMiddleware"],
      "imports": ["express", "session"],
      "dependencies": ["./auth/session", "./handlers"],
      "loc": 89,
      "modified": "2026-01-27"
    }
  ]
}
```

**LLM reads one file, understands entire codebase structure.**

### 2. Inline Comments (Optional, for Humans)

Optionally embed frontmatter in source files, using each language's native comment syntax:

**TypeScript/JavaScript:**
```typescript
// --- FMM ---
// fmm: v0.2
// file: src/auth/session.ts
// exports: [createSession, validateSession, destroySession]
// imports: [jwt, redis-client]
// dependencies: [./types, ./config]
// loc: 234
// modified: 2026-01-27
// ---
```

**Python:**
```python
# --- FMM ---
# fmm: v0.2
# file: src/processor.py
# exports: [DataProcessor, fetch_data, transform]
# imports: [pandas, requests]
# dependencies: [.utils, ..models]
# loc: 156
# python:
#   decorators: [property, staticmethod]
# ---
```

**Rust:**
```rust
// --- FMM ---
// fmm: v0.2
// file: src/lib.rs
// exports: [Config, Pipeline, process]
// imports: [anyhow, serde, tokio]
// dependencies: [crate, super]
// loc: 280
// rust:
//   async_functions: 2
//   derives: [Clone, Debug, Deserialize, Serialize]
//   lifetimes: ['a, 'static]
//   trait_impls: [Display for Error]
//   unsafe_blocks: 1
// ---
```

This is secondary - useful when humans read code or for tools that process files individually.

## LLM Integration Patterns

### Pattern 1: Manifest Query (Recommended)

```
LLM receives: "Here's the codebase manifest. Find files related to authentication."
LLM queries: manifest.json (1,500 tokens)
LLM returns: "src/auth/session.ts, src/api/middleware.ts"

Cost: 1,500 tokens instead of 50,000
```

### Pattern 2: Selective File Load

```
LLM reads: manifest.json
LLM decides: "I need session.ts and types.ts"
LLM reads: Only those 2 files

Cost: 1,500 + 600 = 2,100 tokens instead of 50,000
```

### Pattern 3: Context-Aware Prompts

```
System prompt includes: manifest.json
Every subsequent query: LLM already knows codebase structure

Cost: One-time load, amortized across session
```

## Installation

```bash
# From source (requires Rust)
cargo install --path .

# Or use directly
cargo run -- generate src/
```

## Quick Start

```bash
# Initialize configuration
fmm init

# Generate manifest + optional inline frontmatter
fmm generate src/

# Output manifest only (no file modifications)
fmm generate --manifest-only src/

# Update all frontmatter (regenerate from current code)
fmm update src/

# Validate frontmatter is up to date (useful for CI)
fmm validate src/

# Dry run (see what would change)
fmm generate --dry-run src/
```

## Configuration

Create `.fmmrc.json` in your project root:

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024,
  "manifest_path": ".fmm/manifest.json",
  "inline_comments": false
}
```

**Note:** `inline_comments: false` generates manifest only. Set to `true` if you want embedded frontmatter for human readability.

## Supported Languages

| Language | Extensions | Exports | Imports | Dependencies | Custom Fields |
|----------|-----------|---------|---------|-------------|---------------|
| TypeScript | `.ts`, `.tsx` | Functions, classes, interfaces, variables | Package imports | Relative imports | - |
| JavaScript | `.js`, `.jsx` | Functions, classes, variables | Package imports | Relative imports | - |
| Python | `.py` | Functions, classes, constants, `__all__` | External packages | Relative imports | `decorators` |
| Rust | `.rs` | `pub` items (excludes `pub(crate)`) | External crates (excludes `std`) | `crate::`, `super::` | `derives`, `unsafe_blocks`, `trait_impls`, `lifetimes`, `async_functions` |
| Go | `.go` | Capitalized functions, types, consts, vars | Standard library packages | External modules (e.g., `github.com/...`) | - |
| Java | `.java` | Classes, interfaces, enums, public methods | Root packages | Full import paths | `annotations` |
| C++ | `.cpp`, `.hpp`, `.cc`, `.hh`, `.cxx`, `.hxx` | Functions, classes, structs, enums, templates | System headers (`<...>`) | Local headers (`"..."`) | `namespaces` |
| C# | `.cs` | Public classes, interfaces, structs, enums, methods | `using` namespaces | - | `namespaces`, `attributes` |
| Ruby | `.rb` | Classes, modules, top-level methods | `require` gems | `require_relative` paths | `mixins` |

**10 languages = ~95% GitHub codebase coverage.**

### Language-Specific Fields

**Python** includes a `python:` section with:
- `decorators` - List of decorators used (e.g., `staticmethod`, `property`, `app.route`)

**Rust** includes a `rust:` section with:
- `derives` - Derive macros used (e.g., `Debug`, `Clone`, `Serialize`)
- `unsafe_blocks` - Count of `unsafe` blocks
- `trait_impls` - Trait implementations (e.g., `Display for Error`)
- `lifetimes` - Lifetime parameters used (e.g., `'a`, `'static`)
- `async_functions` - Count of `async fn` declarations

**Java** includes a `java:` section with:
- `annotations` - Annotations used (e.g., `Service`, `Override`, `Deprecated`)

**C++** includes a `cpp:` section with:
- `namespaces` - Namespace definitions (e.g., `engine`, `utils`)

**C#** includes a `csharp:` section with:
- `namespaces` - Namespace declarations (e.g., `MyApp.Services`)
- `attributes` - Attributes used (e.g., `Serializable`, `Required`)

**Ruby** includes a `ruby:` section with:
- `mixins` - Included/extended/prepended modules (e.g., `Comparable`, `Enumerable`)

## The Economics

### Token Cost Analysis

| Model | Cost per 1M tokens | 100-file scan without fmm | With fmm manifest |
|-------|-------------------|--------------------------|-------------------|
| Claude Opus 4.5 | $5.00 input | $0.25 | $0.008 |
| Claude Sonnet 4.5 | $3.00 input | $0.15 | $0.005 |
| GPT-4o | $2.50 input | $0.13 | $0.004 |
| Gemini 3 Pro | $2.00 input | $0.10 | $0.003 |
| GPT-5 | $1.25 input | $0.06 | $0.002 |

**At scale:** A coding assistant that scans your codebase 100 times/day saves $6-25/day on a 100-file project.

### Context Window Economics

| Without fmm | With fmm |
|-------------|----------|
| 50K tokens to understand structure | 1.5K tokens |
| 78K tokens left for reasoning | 126.5K tokens for reasoning |
| LLM spends capacity parsing | LLM spends capacity solving |

## How It Works

1. **Parse:** Uses tree-sitter to parse code into AST
2. **Extract:** Identifies exports, imports, dependencies
3. **Generate:** Creates `.fmm/index.json` manifest (primary) and inline comments (optional)
4. **Query:** LLMs use manifest for navigation, read files only when needed

## Performance

- **Speed:** ~1,500 files/second on Apple Silicon (benchmarked with Criterion)
- **Single file parse:** <1ms per file (TypeScript, Python, Rust)
- **Batch 1000 files:** ~670ms total
- **Parallel:** Processes files in parallel (all CPU cores via rayon)
- **Incremental:** Only updates files that changed
- **Memory:** Constant memory usage (streams files)

Run benchmarks yourself: `cargo bench`

## CI/CD Integration

### Pre-Commit Hook

Keep manifest in sync:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: fmm-update
        name: Update fmm manifest
        entry: fmm update
        language: system
        pass_filenames: true
```

### CI Validation

```yaml
# .github/workflows/ci.yml
- name: Validate fmm manifest
  run: |
    cargo install --path .
    fmm validate src/
```

## Comparison

| Tool | Manifest JSON | Auto-Generated | LLM-Queryable | Token Efficient |
|------|---------------|----------------|---------------|-----------------|
| **fmm** | `.fmm/manifest.json` | Fully automatic | Purpose-built | 88-97% reduction |
| Repomix | `.llm` file | Manual trigger | Generic | Variable |
| TypeDoc | HTML/JSON | Build step | Not optimized | N/A |
| JSDoc | Inline only | Manual | Not structured | N/A |

## How It Works Internally

1. **Parse:** Uses tree-sitter to parse code into AST
2. **Extract:** Identifies exports, imports, dependencies
3. **Generate:** Creates structured manifest JSON
4. **Optionally:** Embeds YAML frontmatter in source files

## Claude Code Integration

### MCP Server

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
- `fmm_find_export(name)` - Find file by export name
- `fmm_list_exports(file)` - List exports from a file
- `fmm_search(query)` - Search manifest with filters
- `fmm_get_manifest()` - Get full project structure
- `fmm_file_info(file)` - Get file metadata

### Search CLI

```bash
fmm search --export validateUser    # Find file by export
fmm search --imports crypto         # Files importing crypto
fmm search --loc ">500"             # Large files
fmm search --depends-on ./types     # Files depending on module
fmm search --json                   # Output as JSON
```

## Roadmap

- [x] TypeScript/JavaScript support
- [x] CLI with generate/update/validate
- [x] Parallel processing
- [x] Configuration file
- [x] Manifest JSON output
- [x] Search CLI
- [x] MCP server (LLMs query manifest directly)
- [x] Python support (tree-sitter-python)
- [x] Rust support (tree-sitter-rust)
- [x] Go support (tree-sitter-go)
- [x] Java support (tree-sitter-java)
- [x] C++ support (tree-sitter-cpp)
- [x] C# support (tree-sitter-c-sharp)
- [x] Ruby support (tree-sitter-ruby)
- [ ] Watch mode (auto-update on save)
- [ ] Complexity metrics (cyclomatic complexity)
- [ ] VS Code extension

## Contributing

PRs welcome! Especially for:
- New language support
- Manifest format improvements
- LLM integration examples
- Token reduction benchmarks

## License

MIT

## Philosophy

> LLMs are the developers now. Humans cannot compete on code comprehension speed.
>
> fmm is infrastructure that makes LLMs faster and cheaper. Inline comments are a courtesy to humans. The manifest is the product.

---

Built as part of the [mdcontext](https://github.com/mdcontext/mdcontext) project.

Created by Stuart Robinson (@srobinson) with research assistance from Claude.
