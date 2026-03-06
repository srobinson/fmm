---
name: fmm-navigate
description: "This project uses fmm (.fmmrc.json) for code metadata. INVOKE THIS SKILL before reading or searching source files — it provides the MCP-first navigation protocol that replaces grep/read with O(1) lookups."
---

# fmm — MCP-First Code Navigation

This codebase has FMM metadata available via MCP tools. Use them for instant, structured lookups instead of grep/read.

## MCP Tools (ALWAYS USE THESE FIRST)

| Tool | Use Case | When to Use |
|------|----------|-------------|
| `mcp__fmm__fmm_list_files` | Orient to codebase structure | Start here — highest blast-radius first with sort_by: "downstream" |
| `mcp__fmm__fmm_lookup_export` | Find where a symbol is defined | O(1) lookup — replaces grep for symbol location |
| `mcp__fmm__fmm_file_outline` | Understand file structure | Table-of-contents before reading anything |
| `mcp__fmm__fmm_read_symbol` | Read source for a symbol | Surgical extraction — one call, exact lines |
| `mcp__fmm__fmm_list_exports` | Fuzzy export discovery | Find exports matching a pattern or in a directory |
| `mcp__fmm__fmm_dependency_graph` | Impact analysis | Upstream deps + downstream blast radius |
| `mcp__fmm__fmm_search` | Multi-criteria search | AND queries: imports X, size > N, exports Y |
| `mcp__fmm__fmm_glossary` | Symbol-level blast radius | All definitions + who imports each — before renaming |

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

Universal codebase search. Use 'term' for smart search across exports, files, and imports. Use structured filters (export, imports, depends_on, LOC) for precise queries. Combine 'term' with filters to narrow results with AND semantics — only exports matching the term from files matching the filters are returned. Note: depends_on uses transitive matching (full import chain), not direct-only. For direct importers only, use fmm_dependency_graph with depth=1.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `term` | string | no | Universal search term — searches exports (exact then fuzzy), file paths, and imports. Returns grouped results. Can ... |
| `export` | string | no | Find files exporting this symbol (exact match, then case-insensitive substring fallback) |
| `imports` | string | no | Find all files that import this package/module (substring match) |
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

1. **`fmm_list_files(sort_by: "downstream")` IS YOUR STARTING POINT** — always orient before diving in.
2. **`fmm_read_symbol` IS YOUR DEFAULT** — need to see code? Use it. One call, exact lines, zero waste.
3. **`fmm_file_outline` BEFORE READING** — see the shape of a file before deciding what to read.
4. **MCP TOOLS ARE PRIMARY** — always call `fmm_*` tools before grep/read.
5. **STRUCTURED > UNSTRUCTURED** — MCP returns parsed data, grep returns text.
6. **READ SOURCE ONLY WHEN EDITING** — sidecars/MCP tell you what you need for navigation.

## Sidecar Fallback

If MCP tools are unavailable, sidecar files exist at `filename.ext.fmm`:

```yaml
file: src/core/pipeline.ts
fmm: v0.3
exports:
  createPipeline: [10, 45]
  PipelineConfig: [47, 52]
imports: [zod, lodash]
dependencies: [./engine, ./validators, ../utils/logger]
loc: 142
```

Use `Grep "exports:" **/*.fmm` as fallback when MCP is not available.
