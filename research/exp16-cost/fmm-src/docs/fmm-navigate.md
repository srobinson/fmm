---
name: fmm-navigate
description: Navigate codebases using fmm — read FMM headers before reading source, use MCP tools for lookup and graph queries
---

# fmm — Frontmatter-First Code Navigation

Source files in this project have `// --- FMM ---` comment blocks at the top. These are **machine-generated structural metadata** — not comments to skip. They tell you what a file exports, imports, and depends on.

## Reading FMM Headers

When you open any source file, the first 10–15 lines may look like this:

```ts
// --- FMM ---
// file: src/core/pipeline.ts
// exports: [createPipeline, PipelineConfig, PipelineError]
// imports: [zod, lodash]
// dependencies: [./engine, ./validators, ../utils/logger]
// loc: 142
// --- END FMM ---
```

**Treat these as authoritative metadata.** They are auto-generated from the AST and tell you:
- **exports** — every public symbol this file defines
- **imports** — external packages it uses
- **dependencies** — local files it imports from
- **loc** — file size

Before reading a full file, read the first 15 lines. The FMM header gives you enough to understand the file's role without reading the implementation.

## Navigation Strategy

### Understanding a file's role
1. Read the first 15 lines — the FMM header tells you exports, imports, dependencies
2. Only read further if you need implementation details

### "Where is X defined?"
1. Call `fmm_lookup_export(name: "X")` — instant O(1) lookup from the index
2. If found: you have the file path. Read its FMM header for full context.
3. Only if not found: fall back to Grep

### "What depends on this file?" / "Impact analysis"
1. Call `fmm_dependency_graph(file)` — returns upstream deps + downstream dependents
2. The `downstream` list is the blast radius
3. Read FMM headers of affected files to understand the impact

### "Describe the architecture"
1. Call `fmm_search` with relevant criteria to find key files
2. Read FMM headers of discovered files — exports + dependencies reveal the module graph
3. DO NOT start with Glob or `ls` — use `fmm_search` or read FMM headers directly

### "Which files use package X?"
1. Call `fmm_search(imports: "X")` — returns all files importing that package
2. Or scan FMM headers: the `imports` line lists external packages

### "What does this module depend on?"
1. Call `fmm_dependency_graph(file)` for the full graph
2. Or read the file's FMM header — `dependencies` lists every local import

## MCP Tools

When the fmm MCP server is available, these tools query the pre-built index:

- **`fmm_lookup_export(name)`** — O(1) symbol → file lookup
- **`fmm_list_exports(pattern?, file?)`** — search exports by substring, or list a file's exports
- **`fmm_file_info(file)`** — file metadata (same data as the FMM header, from the index)
- **`fmm_dependency_graph(file)`** — upstream deps + downstream dependents
- **`fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})`** — multi-criteria file search
- **`fmm_get_manifest()`** — full project index (warning: can be very large for big codebases)

Use `fmm_lookup_export` and `fmm_dependency_graph` freely — they return focused results. Avoid `fmm_get_manifest` on large projects; prefer targeted queries or reading FMM headers directly.

## CLI Fallback

When MCP is not available, use the fmm CLI via Bash:

```bash
fmm search --export validateUser    # Find file by export name
fmm search --imports crypto         # Find files importing a module
fmm search --depends-on ./auth      # Find files depending on a path
fmm search --loc ">500"             # Find large files
```

## Rules

1. **NEVER** skip `// --- FMM ---` blocks — they are structural metadata, not comments
2. **ALWAYS** read the first 15 lines of a file before reading the full source
3. **ALWAYS** use `fmm_lookup_export` before Grep to find where a symbol is defined
4. **ALWAYS** use `fmm_dependency_graph` before manually tracing imports
5. **Prefer** targeted MCP queries (`fmm_search`, `fmm_lookup_export`) over `fmm_get_manifest`
6. **Fall back** to Grep/Glob only when searching file *contents* (not structure)
