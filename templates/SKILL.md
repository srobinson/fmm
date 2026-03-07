---
name: fmm
description: >
  MCP-first code navigation for this codebase. Use before any symbol lookup,
  file search, dependency trace, impact analysis, or codebase evaluation —
  replaces grep/glob/read with O(1) fmm_* tool calls. Trigger when: starting
  any task involving unfamiliar code, navigating code structure, finding where
  a symbol is defined, checking what imports a file, tracing blast radius
  before a rename, mapping test coverage, or evaluating/auditing a codebase.
---

# FMM — MCP-First Code Navigation

This codebase has FMM metadata available via the **`fmm` MCP server**. All tools are prefixed `fmm_*`. Use them for instant, structured lookups instead of grep/read.

The index stays current throughout your session — a hook re-indexes any file you edit immediately after the write. You can trust fmm data at every point in your task.

## Before You Touch Any Code

If you are about to call `Read`, `Grep`, or `Glob` on a source file — stop. Ask: does fmm answer this? It answers structural questions at O(1): file topology, symbol locations, export maps, dependency graphs, blast radius. Reading files to derive those answers costs 10-50x more tokens and is less complete.

Reserve `Read` for two cases only: editing a specific symbol, or understanding logic that `fmm_read_symbol` cannot provide.

## MCP Tools (ALWAYS USE THESE FIRST)

| Tool                   | Use Case                                                                                                                          | Example                                                              |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `fmm_list_files` | Orient in an unknown codebase. sort_by: loc (heaviest), downstream (most-imported, best pre-refactoring), name, exports, modified | `fmm_list_files(directory: "src/", sort_by: "downstream")` |
| `fmm_file_outline` | Full structural profile — exports, public/private (include_private) methods, line ranges | `fmm_file_outline(file: "src/core/index.ts", include_private: true)` |
| `fmm_lookup_export` | O(1) exact lookup → file, line range, full file profile | `fmm_lookup_export(name: "createPipeline")` |
| `fmm_list_exports` | Export search: substring or regex (auto-detected). `^handle`, `Service$`, `^[A-Z]` for regex; plain text for substring | `fmm_list_exports(pattern: "^[A-Z]", directory: "packages/core/")` |
| `fmm_read_symbol` | Exact source for a named export or specific method | `fmm_read_symbol(name: "Injector.loadInstance")` |
| `fmm_search` | Cross-cutting queries: imports, LOC range, depends_on, term | `fmm_search(imports: "rxjs", min_loc: 500)` |
| `fmm_dependency_graph` | Upstream deps + downstream blast radius. filter: "source" strips test files, filter: "tests" shows only test coverage | `fmm_dependency_graph(file: "src/core/index.ts", filter: "source")` |
| `fmm_glossary` | Symbol impact — call-site callers or test coverage by method | `fmm_glossary(pattern: "Injector.loadInstance", mode: "source")` |

## Navigation Workflow

1. **Orient** — `fmm_list_files(sort_by: "downstream")` — highest blast-radius files first. Start here before touching anything.
2. **Locate** — `fmm_lookup_export("SymbolName")` — O(1) file + line range. Replaces grep.
3. **Outline** — `fmm_file_outline(file, include_private: true)` — full method inventory including private members.
4. **Read** — `fmm_read_symbol("Class.method", line_numbers: true)` — surgical extraction.
5. **Impact** — `fmm_glossary("Class.method")` — confirmed callers before renaming or changing a signature.

### Discovering Code Structure

```
fmm_list_files(sort_by: "downstream")   →  highest blast-radius first
fmm_list_files(group_by: "subdir")      →  directory topology in one call
fmm_list_files(filter: "source")        →  source files only (no tests)
fmm_list_files(pattern: "*.ts")         →  filter by filename glob
```

### Finding a Symbol

```
fmm_lookup_export("SymbolName")         →  O(1) file + line range
fmm_list_exports(pattern: "auth")       →  fuzzy: validateAuth, authMiddleware
fmm_file_outline(file: "src/foo.ts")    →  all exports with line ranges
fmm_file_outline(..., include_private: true)  →  private members too
```

### Reading Code

```
fmm_read_symbol("ClassName")            →  full class source (capped at 10KB)
fmm_read_symbol("Class.method")         →  single method — surgical extraction
fmm_read_symbol("Symbol", line_numbers: true)  →  with absolute line numbers
fmm_read_symbol("LargeClass", truncate: false) →  bypass 10KB cap
```

### Impact Analysis

```
fmm_glossary("loadInstance")                    →  all callers (named-import precision)
fmm_glossary("Injector.loadInstance")           →  call-site precision
fmm_dependency_graph(file: "src/injector.ts")   →  upstream deps + downstream blast radius
fmm_dependency_graph(..., filter: "source")     →  production blast radius (no tests)
fmm_dependency_graph(..., depth: -1)            →  full transitive closure
```

### Searching

```
fmm_search(term: "store")                       →  smart search: exports, files, imports
fmm_search(imports: "lodash", min_loc: 100)     →  structured AND query
fmm_search(export: "createStore", min_loc: 50)  →  export + size filter
fmm_search(depends_on: "src/auth.ts")           →  full blast radius (transitive)
```

## Navigation Protocol

### "Orient me / What's in this directory?"

```
1. fmm_list_files(directory: "packages/core/", sort_by: "loc") → largest files first
2. Top entries = complexity anchors. Use fmm_file_outline on those first.
```

First tool to reach for in an unknown codebase. Default sort is `loc` (heaviest files first).

**Sort modes:** `loc` (default, heaviest files), `downstream` (most-imported — best before a refactor to see blast radius), `exports` (most exported symbols), `name` (alphabetical), `modified` (recently changed).

**Pre-refactoring:** use `sort_by: "downstream"` to find the files other files depend on most. Those are the highest-risk targets for changes.

### "What's in this file?"

```
1. fmm_file_outline(file: "src/foo.ts") → every export + public methods with line ranges
2. Decide WHAT to read before reading anything
```

`fmm_file_outline` lists all public methods on classes with exact line ranges. For a 1,000-line class, you see the full table of contents in one call.

### "Where is X defined?"

```
1. fmm_lookup_export(name: "X") → file, line range, AND full file profile — DONE
2. Not found? → fmm_list_exports(pattern: "X") for fuzzy match
3. Still nothing? → fall back to Grep
```

`fmm_lookup_export` returns more than a location — the entire file's export map, imports, and dependency list come with it.

### "Show me the code for X"

```
1. fmm_read_symbol(name: "ClassName.methodName") → exact method source — DONE
   Full class: fmm_read_symbol(name: "ClassName") — truncates at 10KB; add truncate: false for full source
```

**Always use `ClassName.method` notation for large classes.** It extracts exactly that method — no class body noise. Reading a 1,000-line class to find an 80-line method wastes ~90% of your token budget.

### "Find everything named like X"

```
1. fmm_list_exports(pattern: "X") → all matching exports with file + line range
2. Scope: fmm_list_exports(pattern: "X", directory: "packages/core/")
```

Results include class methods (e.g., `Injector.loadInstance`) as distinct entries. Use `offset` to paginate wide searches.

### "Cross-cutting query: files using X with more than N lines"

```
1. fmm_search(imports: "rxjs", min_loc: 500) → files matching ALL criteria with full metadata
2. fmm_search(depends_on: "src/core/injector.ts") → all files in the transitive dependency chain
3. fmm_search(term: "Injector") → EXPORTS + FILES + IMPORTS grouped by type
```

`depends_on` uses **transitive** matching — it returns the full downstream closure, not just direct importers. For direct importers only, use `fmm_dependency_graph(depth: 1)` and read `downstream`.

### "What would break if I rename/change X?"

```
1. fmm_glossary(pattern: "ClassName.method") → actual call sites only — surgical blast radius
   fmm_glossary(pattern: "ClassName") → file-level: all files importing the class's file
2. Separate production vs test impact: mode: "source" | "tests" | "all"
```

**The dotted pattern is the contract.** `fmm_glossary(pattern: "loadInstance")` returns every file that imports `injector.ts` — a superset. `fmm_glossary(pattern: "Injector.loadInstance")` runs a tree-sitter second pass and returns only files with an actual call site. Use the dotted form for rename safety.

### "What tests cover X?"

```
1. fmm_glossary(pattern: "ClassName.method", mode: "tests") → test files with actual call sites
   fmm_glossary(pattern: "ClassName", mode: "tests") → all test files importing the class
```

### "What depends on this file? What does it import?"

```
1. fmm_dependency_graph(file: "src/foo.ts")
   - local_deps: intra-project imports, resolved to actual paths
   - external: third-party packages
   - downstream: files that import this file (complete and reliable)
2. Transitive: fmm_dependency_graph(file: "...", depth: 3) or depth: -1 for full closure
```

### "Evaluate or audit this codebase"

```
1. fmm_list_files(group_by: "subdir")               → full topology, LOC per bucket
2. fmm_list_files(sort_by: "loc", limit: 20)        → largest files = complexity anchors
3. fmm_list_files(sort_by: "downstream", limit: 15) → highest blast-radius files
4. fmm_file_outline on key files                    → structure without reading
5. fmm_search(imports: "package")                   → cross-cutting architecture patterns
```

A comprehensive evaluation in 5-8 calls and under 5k tokens — faster and more complete than reading files.


## Parameter Reference

> Auto-generated from tools.toml — all options for each tool.

### `fmm_lookup_export`

Instant O(1) symbol-to-file lookup. Find where a function, class, type, or variable is defined. Returns the file path plus metadata (exports, imports, dependencies, LOC). Use before Grep.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | yes | Exact export name to find (function, class, type, variable, component) |

### `fmm_list_exports`

Search or list exported symbols across the codebase. Use 'pattern' for fuzzy discovery (e.g. 'auth' matches validateAuth, authMiddleware). Patterns with regex metacharacters (^, $, [, (, \\, ., *, +, ?, {) are compiled as regex. Use 'directory' to scope results to a path prefix (e.g. 'packages/core/'). Use 'file' to list a specific file's exports. Default limit: 200. Use offset to page through large result sets.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | no | Pattern to match against export names. Plain strings use case-insensitive substring match (e.g. 'auth' finds validate... |
| `file` | string | no | File path — returns all exports from this specific file |
| `directory` | string | no | Path prefix to scope results (e.g. 'packages/core/'). Only exports from files under this directory are returned. |
| `limit` | integer | no | Maximum number of results to return (default: 200). Increase for broader listings. |
| `offset` | integer | no | Number of results to skip before returning (default: 0). Use for pagination: offset=200 returns results 201–400. |

### `fmm_dependency_graph`

Get a file's dependency graph: upstream dependencies (what it imports) and downstream dependents (what would break if it changes). Use for impact analysis and blast radius. Add depth>1 for transitive traversal; depth=-1 for full closure. Use filter='source' to exclude test files from downstream, or filter='tests' to see only test coverage.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | yes | File path to analyze — returns all upstream dependencies and downstream dependents |
| `depth` | integer | no | Traversal depth (default: 1 = direct deps only). depth=2 adds transitive deps. depth=-1 computes the full transitive ... |
| `filter` | enum: all \| source \| tests | no | Filter upstream and downstream lists by file type. 'all' (default): no filtering. 'source': exclude test files (*.spe... |

### `fmm_read_symbol`

Read the source code for a specific exported symbol. Returns the exact lines where the function/class/type is defined, without reading the entire file. Requires line-range data from v0.3 sidecars. Use `ClassName.method` notation to read a specific public or private method: `fmm_read_symbol(name: "Injector.loadInstance")`. Private methods discovered via fmm_file_outline(include_private: true) are accessible using the same dotted notation. For large symbols (>10KB) use truncate: false to get the full source. Use line_numbers: true to prepend absolute line numbers to each source line.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | yes | Exact export name to read (function, class, type, component), or ClassName.method for a specific public or private me... |
| `truncate` | boolean | no | Whether to apply the 10KB response cap (default: true). Set to false to return the full source for large symbols that... |
| `line_numbers` | boolean | no | When true, prepend absolute line numbers (right-aligned) to each source line. Useful when referencing specific lines ... |

### `fmm_file_outline`

Get a spatial outline of a file: every exported symbol with its line range and size. Like a table-of-contents for the file. Use to understand file structure before reading specific symbols. Set include_private: true to also show private/protected members under each class (TypeScript and Python; on-demand tree-sitter parse).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | yes | File path to outline — returns all exports with line ranges and sizes |
| `include_private` | boolean | no | When true, include private/protected methods and fields under each class, annotated with '# private'. On-demand tree-... |

### `fmm_search`

Universal codebase search. Use 'term' for smart search across codebase-defined exports, file paths, and import names. Note: term searches exports DEFINED in this codebase — it will not find call sites of externally-imported functions (e.g. createServerFn from @tanstack/react-start). For files that call an external function, use imports: package-name. Use structured filters (export, imports, depends_on, LOC) for precise queries. Combine 'term' with filters to narrow results with AND semantics. Note: depends_on uses transitive matching (full import chain), not direct-only. For direct importers only, use fmm_dependency_graph with depth=1.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `term` | string | no | Search codebase-defined exports (exact then fuzzy), file paths, and external import names. Does NOT find call sites o... |
| `export` | string | no | Find files exporting this symbol (exact match, then case-insensitive substring fallback) |
| `imports` | string | no | Find all files that import an external package (npm, pip, crate, etc.) — substring match on the import name. For lo... |
| `depends_on` | string | no | Find all files that transitively depend on this local path (full import chain, not just direct importers) — use for... |
| `min_loc` | integer | no | Minimum lines of code — find files larger than this |
| `max_loc` | integer | no | Maximum lines of code — find files smaller than this |
| `limit` | integer | no | Maximum number of fuzzy export results (default: 50). Increase for broader searches. |

### `fmm_list_files`

List all indexed files under a directory prefix. The first tool to reach for when exploring an unknown module or package. Returns file paths with LOC, export count, and downstream dependent count. Default sort: LOC descending (largest files first). sort_by options: loc (default), name, exports, downstream (blast-radius sort), modified (most recently changed first). Default limit: 200. Use offset to page through large directories.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `directory` | string | no | Directory prefix to filter files (e.g. 'src/cli/' or 'libs/agno/models'). Omit to list all indexed files. |
| `pattern` | string | no | Glob pattern to filter by filename within the directory (e.g. '*.py', '*.rs', 'test_*'). Supports * wildcard. |
| `limit` | integer | no | Maximum number of files to return (default: 200). Increase for broader listings. |
| `offset` | integer | no | Number of files to skip before returning results (default: 0). Use for pagination: offset=200 returns files 201–400. |
| `sort_by` | enum: name \| loc \| exports \| downstream \| modified | no | Sort field. 'loc' (default): lines of code descending. 'name': alphabetical. 'exports': export count descending. 'dow... |
| `order` | enum: asc \| desc | no | Sort order. Defaults: 'name' → asc, 'loc'/'exports'/'downstream' → desc. Explicit 'asc'/'desc' overrides the defa... |
| `group_by` | enum: subdir | no | Collapse files into directory buckets. 'subdir': group by immediate subdirectory, showing file count and total LOC pe... |
| `filter` | enum: all \| source \| tests | no | File type filter. 'all' (default): no filtering. 'source': exclude test files. 'tests': return only test files. Detec... |

### `fmm_glossary`

Symbol-level impact analysis. Given a symbol name or pattern, returns all definitions and exactly which files import each one. Three-layer precision: bare name returns named-import filtered callers (Layer 2, default); dotted name (e.g. 'Injector.loadInstance') adds call-site precision; precision: 'call-site' adds Layer 3 tree-sitter to remove dead imports and annotate re-exports. Use before renaming or changing a signature.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | yes | Required. Case-insensitive substring filter on export name. Bare name (e.g. 'loadInstance') returns named-import filt... |
| `limit` | integer | no | Max entries returned (default 10, hard cap at 50). Use a specific pattern to stay under the default. |
| `mode` | enum: source \| tests \| all | no | source (default): excludes test symbols and test files. tests: only test exports. all: unfiltered. |
| `precision` | enum: named \| call-site | no | named (default): Layer 2 only, fast index lookup with no file reads. call-site: adds Layer 3 tree-sitter verification... |

## Rules

1. **Never use `Read` to understand structure** — use `fmm_file_outline`
2. **Never use `Grep` to find a symbol** — use `fmm_lookup_export` or `fmm_glossary`
3. **Never use `Glob` to explore a directory** — use `fmm_list_files`
4. **`fmm_list_files` first** — orient before navigating
5. **`fmm_file_outline` before reading** — see the shape, then decide what to read
6. **`fmm_read_symbol("ClassName.method")`** — never read a full class to find one method
7. **Dotted pattern for rename safety** — `fmm_glossary("ClassName.method")` for call-site precision
8. **Read source only when editing** — MCP tools tell you everything you need for navigation
9. **Trust the index** — it updates automatically after every file write

## Sidecar Fallback

If MCP tools are unavailable, `.fmm` sidecar files exist alongside source files:

```yaml
file: src/core/pipeline.ts
fmm: v0.3+0.1.11
exports:
  createPipeline: [10, 45]
  PipelineConfig: [47, 52]
imports: [./engine, ./validators, lodash, zod]
loc: 142
modified: 2026-03-06
```

Line ranges enable surgical reads: `Read(file, offset=10, limit=36)`.
