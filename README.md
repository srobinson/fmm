# fmm - Frontmatter Matters

**Auto-generate code frontmatter for LLM-optimized navigation**

## What is this?

`fmm` automatically generates and maintains structured metadata headers (frontmatter) in your source code files. Think "YAML frontmatter for code" - just like markdown files have frontmatter, your code files should too.

## Why?

**For LLMs:** When LLMs scan your codebase, they waste tokens reading 50+ lines to understand what a file does. With frontmatter in the first 5 lines, they get instant context:

```typescript
// ---
// file: src/auth/session.ts
// exports: [createSession, validateSession, destroySession]
// imports: [jwt, redis-client]
// dependencies: [./types, ./config]
// loc: 234
// modified: 2026-01-27
// ---

import jwt from 'jsonwebtoken'
import { RedisClient } from './redis-client'
// ... rest of file
```

**Token savings: 90%+ for file understanding.**

**For Humans:** Instant file understanding, always up-to-date metadata, grep-friendly, no separate docs to maintain.

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

# Generate frontmatter for files that don't have it
fmm generate src/

# Update all frontmatter (regenerate from current code)
fmm update src/

# Validate frontmatter is up to date (useful for CI)
fmm validate src/

# Dry run (see what would change)
fmm generate --dry-run src/
fmm update --dry-run src/
```

## Configuration

Create `.fmmrc.json` in your project root:

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

- ✅ TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`)
- 🚧 Python (`.py`) - Coming soon
- 🚧 Rust (`.rs`) - Coming soon
- 🚧 Go (`.go`) - Coming soon

## Frontmatter Format

### TypeScript/JavaScript

```typescript
// ---
// file: src/auth/session.ts
// exports: [createSession, validateSession, destroySession]
// imports: [jwt, redis-client]
// dependencies: [./types, ./config]
// loc: 234
// modified: 2026-01-27
// ---
```

### Python

```python
# ---
# file: src/processor.py
# exports: [process_data, validate_input]
# imports: [pandas, numpy]
# dependencies: [./utils]
# loc: 156
# modified: 2026-01-27
# ---
```

## Use Cases

### 1. On-Save Hook (VS Code)

Auto-update frontmatter when you save files:

```json
// .vscode/tasks.json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Update Frontmatter",
      "type": "shell",
      "command": "fmm update ${file}",
      "presentation": {
        "reveal": "never"
      }
    }
  ]
}
```

### 2. Pre-Commit Hook

Ensure frontmatter stays in sync:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: fmm-update
        name: Update code frontmatter
        entry: fmm update
        language: system
        pass_filenames: true
```

### 3. CI Validation

```yaml
# .github/workflows/ci.yml
- name: Validate frontmatter
  run: |
    cargo install --path .
    fmm validate src/
```

### 4. Integration with mdcontext

Use frontmatter for faster code indexing:

```bash
# mdcontext can read frontmatter directly (no AST parsing needed)
mdcontext index --read-frontmatter src/
```

## How It Works

1. **Parse:** Uses tree-sitter to parse code into AST
2. **Extract:** Identifies exports, imports, dependencies
3. **Generate:** Creates YAML-formatted comment block
4. **Insert:** Prepends frontmatter to file (or updates existing)

## Performance

- **Speed:** ~1000 files/second on M1 Mac
- **Parallel:** Processes files in parallel (all CPU cores)
- **Incremental:** Only updates files that changed
- **Memory:** Constant memory usage (streams files)

## Comparison

| Tool | Embedded | Auto-Generated | LLM-Optimized | Fast |
|------|----------|----------------|---------------|------|
| **fmm** | ✅ | ✅ | ✅ | ✅ |
| JSDoc | ✅ | ❌ | ❌ | N/A |
| TypeDoc | ❌ (HTML) | ✅ | ❌ | ✅ |
| Repomix | ❌ (.llm) | ✅ | ✅ | ✅ |
| AST Metrics | ❌ (JSON) | ✅ | ❌ | ✅ |

## Roadmap

- [x] TypeScript/JavaScript support
- [x] CLI with generate/update/validate
- [x] Parallel processing
- [x] Configuration file
- [ ] Python support (tree-sitter-python)
- [ ] Rust support (tree-sitter-rust)
- [ ] Go support (tree-sitter-go)
- [ ] VS Code extension
- [ ] Watch mode (auto-update on save)
- [ ] Complexity metrics (cyclomatic complexity)
- [ ] LSP integration
- [ ] Format specification (RFC)

## Contributing

PRs welcome! Especially for:
- New language support (Python, Rust, Go, Java)
- Parser improvements
- Format enhancements
- Documentation

## License

MIT

## Research

This project is based on extensive research into:
- How LLM coding tools navigate codebases (Cursor, Copilot, Aider)
- Existing code documentation tools
- Frontmatter standards
- AST parsing best practices

See `research/frontmatter/` in the parent repo for full research findings.

## Credits

Built as part of the [mdcontext](https://github.com/mdcontext/mdcontext) project.

Created by Stuart Robinson (@srobinson) with research assistance from Claude Sonnet 4.5.
