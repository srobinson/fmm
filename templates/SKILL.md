---
name: fmm-navigate
description: "This project uses fmm (.fmmrc.json) for code metadata. INVOKE THIS SKILL before reading or searching source files — it provides the MCP-first navigation protocol that replaces grep/read with O(1) lookups."
---

# fmm — MCP-First Code Navigation

This codebase has FMM metadata available via MCP tools. Use them for instant, structured lookups instead of grep/read.

## MCP Tools (ALWAYS USE THESE FIRST)

| Tool | Use Case | Example |
|------|----------|---------|
| `mcp__fmm__fmm_read_symbol` | "Show me the code for X" | `fmm_read_symbol(name: "createPipeline")` |
| `mcp__fmm__fmm_lookup_export` | "Where is X defined?" | `fmm_lookup_export(name: "createPipeline")` |
| `mcp__fmm__fmm_file_outline` | "What's in this file?" | `fmm_file_outline(file: "src/core/index.ts")` |
| `mcp__fmm__fmm_list_files` | "What files are in this module?" | `fmm_list_files(path: "src/agent/")` |
| `mcp__fmm__fmm_list_exports` | "Find exports matching X" | `fmm_list_exports(pattern: "swarm")` |
| `mcp__fmm__fmm_dependency_graph` | "Deps and blast radius for this file" | `fmm_dependency_graph(file: "src/core/index.ts")` |
| `mcp__fmm__fmm_file_info` | "Quick file summary" | `fmm_file_info(file: "src/utils/helpers.ts")` |
| `mcp__fmm__fmm_search` | Multi-criteria search with relevance ranking | `fmm_search(imports: "lodash", min_loc: 100)` |

## Navigation Protocol

### "Show me the code for X" (most common)

```
1. Call mcp__fmm__fmm_read_symbol(name: "X")
2. Returns exact source code + file path + line range — DONE
3. No need to find the file, open it, or scan for the symbol
```

**This replaces 3+ tool calls** (search → find file → read file → locate symbol) with ONE call.

Re-export chains are resolved automatically: if `X` is re-exported via `__init__.py` or `index.ts`, the tool follows the chain to the concrete definition.

### "What files are in this module?"

```
1. Call mcp__fmm__fmm_list_files(path: "src/agent/")
2. Returns all indexed files under that path with LOC and export count — DONE
3. Use fmm_file_outline on specific files to understand their shape
```

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

### "What files depend on this file?" / "What does this file import?"

```
1. Call mcp__fmm__fmm_dependency_graph(file: "src/foo.ts")
2. Response includes three fields:
   - local_deps: intra-project files it imports, resolved to actual paths
   - external: third-party packages
   - downstream: files that import this file (blast radius if it changes)
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
| Read a function's source | `fmm_read_symbol` | Exact source, one call; follows re-exports automatically |
| Find symbol definition | `fmm_lookup_export` | O(1) lookup, returns file + line range |
| Understand file structure | `fmm_file_outline` | Shows every export with size, before you read |
| Explore a directory/module | `fmm_list_files` | All files under a path with LOC and export counts |
| Find similar exports | `fmm_list_exports` | Pattern search across all files |
| Impact analysis | `fmm_dependency_graph` | local_deps, external packages, downstream dependents |
| Quick file summary | `fmm_file_info` | Exports/imports/LOC without reading |
| Complex queries | `fmm_search` | Combine filters with relevance ranking (imports X, size > N) |

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
fmm: v0.3+0.1.11
exports:
  createPipeline: [10, 45]
  PipelineConfig: [47, 52]
  PipelineError: [54, 58]
imports: [./engine, ./validators, ../utils/logger, lodash, zod]
loc: 142
modified: 2026-03-05
```

Line ranges let you do surgical reads: `Read(file, offset=10, limit=36)` for just `createPipeline`.

Use `Grep "exports:" **/*.fmm` as fallback when MCP is not available.

## Rules

1. **`fmm_read_symbol` IS YOUR DEFAULT** — Need to see code? Use it. One call, exact lines, zero waste.
2. **`fmm_file_outline` BEFORE READING** — See the shape of a file before deciding what to read.
3. **MCP TOOLS ARE PRIMARY** — Always call `fmm_*` tools before grep/read.
4. **STRUCTURED > UNSTRUCTURED** — MCP returns parsed JSON, grep returns text.
5. **READ SOURCE ONLY WHEN EDITING** — Sidecars/MCP tell you what you need for navigation.
