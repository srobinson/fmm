---
name: fmm-navigate
description: Navigate codebases using FMM MCP tools for O(1) symbol lookups and dependency graphs. ALWAYS use MCP tools before grep/read.
---

# fmm — MCP-First Code Navigation

This codebase has FMM metadata available via MCP tools. Use them for instant, structured lookups instead of grep/read.

## MCP Tools (ALWAYS USE THESE FIRST)

| Tool | Use Case | Example |
|------|----------|---------|
| `mcp__fmm__fmm_lookup_export` | "Where is X defined?" | `fmm_lookup_export(name: "createPipeline")` |
| `mcp__fmm__fmm_dependency_graph` | "What depends on this file?" | `fmm_dependency_graph(file: "src/core/index.ts")` |
| `mcp__fmm__fmm_list_exports` | "Find exports matching X" | `fmm_list_exports(pattern: "swarm")` |
| `mcp__fmm__fmm_file_info` | "What does this file export?" | `fmm_file_info(file: "src/utils/helpers.ts")` |
| `mcp__fmm__fmm_search` | Multi-criteria search | `fmm_search(imports: "lodash", min_loc: 100)` |

## Navigation Protocol

### "Where is X defined?"

```
1. Call mcp__fmm__fmm_lookup_export(name: "X")
2. If found → you have the file path, DONE
3. If not found → try mcp__fmm__fmm_list_exports(pattern: "X") for fuzzy match
4. Only then fall back to Grep on source files
```

**DO NOT** start with `Grep "export.*X"` — use the MCP tool.

### "What files depend on this file?"

```
1. Call mcp__fmm__fmm_dependency_graph(file: "src/foo.ts")
2. Response includes "downstream" array with all dependents
3. DONE — no need to grep or read any files
```

### "Find all exports related to X"

```
1. Call mcp__fmm__fmm_list_exports(pattern: "X")
2. Response lists all matching exports with their files
3. DONE
```

### "What does this module export?"

```
1. Call mcp__fmm__fmm_file_info(file: "src/module/index.ts")
2. Response includes exports, imports, dependencies, LOC
3. DONE — no need to read the source file
```

### "Describe the architecture"

```
1. Call mcp__fmm__fmm_list_exports() to get all exports
2. Call mcp__fmm__fmm_dependency_graph on key entry points
3. Build mental model from structured responses
4. Only read source files for specific implementation details
```

## When to Use Each Tool

| Task | Tool | Why |
|------|------|-----|
| Find symbol definition | `fmm_lookup_export` | O(1) lookup, returns file + metadata |
| Find similar exports | `fmm_list_exports` | Pattern search, returns all matches |
| Impact analysis | `fmm_dependency_graph` | Pre-computed upstream/downstream |
| File summary | `fmm_file_info` | Exports/imports/LOC without reading |
| Complex queries | `fmm_search` | Combine filters (imports X, size > N) |

## When to Fall Back to Grep/Read

Only use Grep/Read when:
- MCP tool returns no results
- You need to search inside function bodies (not just exports)
- You need to understand implementation details before editing
- You need to read test files or documentation

## Sidecar Files (Fallback)

If MCP tools are unavailable, sidecar files exist at `filename.ext.fmm`:

```yaml
file: src/core/pipeline.ts
fmm: v0.2
exports: [createPipeline, PipelineConfig, PipelineError]
imports: [zod, lodash]
dependencies: [./engine, ./validators, ../utils/logger]
loc: 142
```

Use `Grep "exports:.*SymbolName" **/*.fmm` as fallback when MCP is not available.

## Rules

1. **MCP TOOLS ARE PRIMARY** — Always call `fmm_*` tools before grep/read
2. **STRUCTURED > UNSTRUCTURED** — MCP returns parsed JSON, grep returns text
3. **ONE CALL > MANY CALLS** — `fmm_dependency_graph` replaces multiple grep/read sequences
4. **READ SOURCE ONLY WHEN EDITING** — Sidecars/MCP tell you what you need for navigation
