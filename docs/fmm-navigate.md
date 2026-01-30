---
name: fmm-navigate
description: Navigate codebases using .fmm sidecar files — read sidecars before source, use MCP tools for lookup and graph queries
---

# fmm — Sidecar-First Code Navigation

Source files in this project have `.fmm` sidecar companions. For every `foo.ts` there may be a `foo.ts.fmm` containing structured metadata — exports, imports, dependencies, and file size.

## Reading Sidecars

Before opening a source file, check if it has a sidecar:

```
# foo.ts.fmm
file: src/core/pipeline.ts
fmm: v0.2
exports: [createPipeline, PipelineConfig, PipelineError]
imports: [zod, lodash]
dependencies: [./engine, ./validators, ../utils/logger]
loc: 142
```

A sidecar tells you everything about a file's role without reading the source. Use this to decide which files you actually need to open.

## Navigation Strategy

### Finding which files to edit
1. `Grep "exports:.*DropColumn" **/*.fmm` — find sidecars mentioning a symbol
2. Read the matching `.fmm` file — get the full metadata (exports, deps, loc)
3. Only then open the source file if you need to edit it

### "Where is X defined?"
1. `Grep "exports:.*X" **/*.fmm` — search sidecar exports
2. Or call `fmm_lookup_export(name: "X")` if MCP is available
3. Only fall back to Grep on source if not found in sidecars

### "What depends on this file?"
1. Call `fmm_dependency_graph(file)` if MCP is available
2. Or `Grep "dependencies:.*filename" **/*.fmm` to find dependents
3. Read sidecars of affected files to understand the blast radius

### "Describe the architecture"
1. `Glob **/*.fmm` to discover all sidecar files
2. Read sidecars to understand the module graph from exports + dependencies
3. DO NOT start by reading source files — sidecars give you the structure

### "Which files use package X?"
1. `Grep "imports:.*X" **/*.fmm` — find all files importing that package
2. Or call `fmm_search(imports: "X")` if MCP is available

## MCP Tools

When the fmm MCP server is available, these tools query the pre-built index:

- **`fmm_lookup_export(name)`** — O(1) symbol -> file lookup
- **`fmm_list_exports(pattern?, file?)`** — search exports by substring
- **`fmm_file_info(file)`** — file metadata from the sidecar
- **`fmm_dependency_graph(file)`** — upstream deps + downstream dependents
- **`fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})`** — multi-criteria search

## Rules

1. **CHECK SIDECARS FIRST** — before reading any source file, check if `filename.fmm` exists
2. **USE SIDECARS TO NAVIGATE** — grep sidecars to find relevant files, not source code
3. **ONLY OPEN SOURCE FILES YOU WILL EDIT** — sidecars tell you the file's role; only read source when you need to see or modify the implementation
4. **USE MCP TOOLS** when available — `fmm_lookup_export` and `fmm_search` are faster than grep
5. **FALL BACK** to Grep/Glob on source only when searching file *contents* (not structure)
