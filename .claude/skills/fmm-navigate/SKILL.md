---
name: fmm-navigate
description: Navigate codebases using FMM MCP tools for O(1) symbol lookups, source reads, and dependency graphs. ALWAYS use MCP tools before grep/read.
---

# fmm — MCP-First Code Navigation

This codebase has FMM metadata available via MCP tools. Use them for instant, structured lookups instead of grep/read.

## MCP Tools (ALWAYS USE THESE FIRST)

| Tool | Use Case | Example |
|------|----------|---------|
| `mcp__fmm__fmm_read_symbol` | "Show me the code for X" | `fmm_read_symbol(name: "createPipeline")` |
| `mcp__fmm__fmm_lookup_export` | "Where is X defined?" | `fmm_lookup_export(name: "createPipeline")` |
| `mcp__fmm__fmm_file_outline` | "What's in this file?" | `fmm_file_outline(file: "src/core/index.ts")` |
| `mcp__fmm__fmm_list_exports` | "Find exports matching X" | `fmm_list_exports(pattern: "swarm")` |
| `mcp__fmm__fmm_dependency_graph` | "What depends on this file?" | `fmm_dependency_graph(file: "src/core/index.ts")` |
| `mcp__fmm__fmm_file_info` | "Quick file summary" | `fmm_file_info(file: "src/utils/helpers.ts")` |
| `mcp__fmm__fmm_search` | Multi-criteria search | `fmm_search(imports: "lodash", min_loc: 100)` |

## Navigation Protocol

### "Show me the code for X" (most common)

```
1. Call mcp__fmm__fmm_read_symbol(name: "X")
2. Returns exact source code + file path + line range — DONE
3. No need to find the file, open it, or scan for the symbol
```

**This replaces 3+ tool calls** (search → find file → read file → locate symbol) with ONE call.

### "Where is X defined?"

```
1. Call mcp__fmm__fmm_lookup_export(name: "X")
2. Returns file path + line range [start, end] — DONE
3. If not found → try mcp__fmm__fmm_list_exports(pattern: "X") for fuzzy match
4. Only then fall back to Grep on source files
```

### "What's the structure of this file?"

```
1. Call mcp__fmm__fmm_file_outline(file: "src/foo.ts")
2. Returns every export with line ranges and sizes
3. Use this to decide WHAT to read before reading anything
```

### "What files depend on this file?"

```
1. Call mcp__fmm__fmm_dependency_graph(file: "src/foo.ts")
2. Response includes "downstream" array with all dependents — DONE
```

### "Describe the architecture"

```
1. Call mcp__fmm__fmm_list_exports() to get all exports
2. Call mcp__fmm__fmm_file_outline on key files to see their shape
3. Call mcp__fmm__fmm_dependency_graph on entry points
4. Only call mcp__fmm__fmm_read_symbol for specific functions you need to understand
```

## When to Use Each Tool

| Task | Tool | Why |
|------|------|-----|
| Read a function's source | `fmm_read_symbol` | Returns exact source code, one call |
| Find symbol definition | `fmm_lookup_export` | O(1) lookup, returns file + line range |
| Understand file structure | `fmm_file_outline` | Shows every export with size, before you read |
| Find similar exports | `fmm_list_exports` | Pattern search across all files |
| Impact analysis | `fmm_dependency_graph` | Pre-computed upstream/downstream |
| Quick file summary | `fmm_file_info` | Exports/imports/LOC without reading |
| Complex queries | `fmm_search` | Combine filters (imports X, size > N) |

## When to Fall Back to Grep/Read

Only use Grep/Read when:
- MCP tool returns no results
- You need to search inside function bodies (not just exports)
- You need to read non-exported code or private functions
- You need to read test files or documentation

## Sidecar Files (Fallback)

If MCP tools are unavailable, sidecar files exist at `filename.ext.fmm`:

```yaml
file: src/core/pipeline.ts
fmm: v0.3
exports:
  createPipeline: [10, 45]
  PipelineConfig: [47, 52]
  PipelineError: [54, 58]
imports: [zod, lodash]
dependencies: [./engine, ./validators, ../utils/logger]
loc: 142
```

Line ranges let you do surgical reads: `Read(file, offset=10, limit=36)` for just `createPipeline`.

Use `Grep "exports:" **/*.fmm` as fallback when MCP is not available.

## Rules

1. **`fmm_read_symbol` IS YOUR DEFAULT** — Need to see code? Use it. One call, exact lines, zero waste.
2. **`fmm_file_outline` BEFORE READING** — See the shape of a file before deciding what to read.
3. **MCP TOOLS ARE PRIMARY** — Always call `fmm_*` tools before grep/read.
4. **STRUCTURED > UNSTRUCTURED** — MCP returns parsed JSON, grep returns text.
5. **READ SOURCE ONLY WHEN EDITING** — Sidecars/MCP tell you what you need for navigation.
