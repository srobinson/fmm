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

Optionally embed frontmatter in source files:

```typescript
// --- FMM ---
// file: src/auth/session.ts
// exports: [createSession, validateSession, destroySession]
// imports: [jwt, redis-client]
// dependencies: [./types, ./config]
// loc: 234
// modified: 2026-01-27
// ---

import jwt from 'jsonwebtoken'
// ... rest of file
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

- TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`)
- Python (`.py`) - Coming soon
- Rust (`.rs`) - Coming soon
- Go (`.go`) - Coming soon

## The Economics

### Token Cost Analysis

| Model | Cost per 1M tokens | 100-file scan without fmm | With fmm manifest |
|-------|-------------------|--------------------------|-------------------|
| GPT-4 | $30 input | $1.50 | $0.045 |
| Claude | $15 input | $0.75 | $0.023 |
| GPT-4o | $5 input | $0.25 | $0.008 |

**At scale:** A coding assistant that scans your codebase 100 times/day saves $40-145/day on a 100-file project.

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

- **Speed:** ~1000 files/second on M1 Mac
- **Parallel:** Processes files in parallel (all CPU cores)
- **Incremental:** Only updates files that changed
- **Memory:** Constant memory usage (streams files)

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
- [ ] Python support (tree-sitter-python)
- [ ] Rust support (tree-sitter-rust)
- [ ] Go support (tree-sitter-go)
- [ ] Watch mode (auto-update on save)
- [ ] Complexity metrics (cyclomatic complexity)
- [ ] VS Code extension

## Contributing

PRs welcome! Especially for:
- New language support (Python, Rust, Go, Java)
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
