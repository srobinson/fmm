# Introduction

**fmm** (Frontmatter Matters) generates `.fmm` sidecar files alongside your source code. Each sidecar is a small YAML file listing the exports, imports, dependencies, and line count of its companion source file.

LLM agents use these sidecars to navigate codebases without reading every source file — reducing token usage by 80-90% while maintaining full structural awareness.

## Why sidecars?

When an LLM agent needs to understand your codebase, it typically reads every source file — consuming tens of thousands of tokens. With fmm sidecars, the agent reads compact metadata instead:

| Approach | Tokens for 500-file project |
|----------|---------------------------|
| Read all source files | ~50,000 tokens |
| Read fmm sidecars | ~2,000 tokens |

That's a **96% reduction** in context usage, with zero loss of navigational capability.

## How it works

1. **Generate** — fmm parses your source files using tree-sitter ASTs and writes `.fmm` sidecar files
2. **Navigate** — LLM agents read sidecars to understand file structure, then open only the files they need
3. **Search** — O(1) reverse-index lookups find which file exports any symbol

## Supported languages

TypeScript, JavaScript, Python, Rust, Go, Java, C++, C#, Ruby

## Quick start

```bash
cargo install fmm
cd your-project
fmm init
```

That's it. Your AI assistant now navigates via metadata.
