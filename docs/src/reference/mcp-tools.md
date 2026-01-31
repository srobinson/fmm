# MCP Tools

fmm exposes a Model Context Protocol (MCP) server that LLM agents can call directly. Start it with `fmm mcp` or configure it in `.mcp.json` via `fmm init --mcp`.

## Tools

### `fmm_lookup_export`

Instant O(1) symbol-to-file lookup. Find where a function, class, type, or variable is defined. Returns the file path plus metadata (exports, imports, dependencies, LOC).

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | yes | Exact export name to find (function, class, type, variable, component) |

**Example request:**
```json
{"name": "createStore"}
```

**Example response:**
```
src/store/index.ts
  exports: createStore, configureStore, StoreConfig
  imports: redux, immer
  dependencies: ../config
  loc: 89
```

### `fmm_list_exports`

Search or list exported symbols across the codebase. Use `pattern` for fuzzy discovery (e.g. `auth` matches `validateAuth`, `authMiddleware`). Use `file` to list a specific file's exports.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | no | Substring to match against export names (case-insensitive) |
| `file` | string | no | File path — returns all exports from this specific file |

### `fmm_file_info`

Get a file's structural profile from the index: exports, imports, dependencies, LOC. Same data as the file's `.fmm` sidecar, but from the pre-built index.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | yes | File path to inspect |

### `fmm_dependency_graph`

Get a file's dependency graph: upstream dependencies (what it imports) and downstream dependents (what would break if it changes). Use for impact analysis and blast radius estimation.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | yes | File path to analyze |

**Example response:**
```
src/auth.ts dependency graph:

Upstream (this file depends on):
  src/config.ts
  src/db.ts

Downstream (depends on this file):
  src/routes/login.ts
  src/routes/register.ts
  src/middleware/session.ts
```

### `fmm_search`

Search files by structural criteria: exported symbol, imported package, local dependency, or LOC range. Filters combine with AND logic.

**Input:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `export` | string | no | Find the file that exports this symbol (exact match) |
| `imports` | string | no | Find all files that import this package/module (substring match) |
| `depends_on` | string | no | Find all files that depend on this local path |
| `min_loc` | integer | no | Minimum lines of code |
| `max_loc` | integer | no | Maximum lines of code |

## Legacy aliases

These aliases are supported for backward compatibility:

| Alias | Maps to |
|-------|---------|
| `fmm_find_export` | `fmm_lookup_export` |
| `fmm_find_symbol` | `fmm_lookup_export` |
| `fmm_file_metadata` | `fmm_file_info` |
| `fmm_analyze_dependencies` | `fmm_dependency_graph` |

## Protocol

- **Version:** 2024-11-05
- **Transport:** stdio
- **Max response size:** 10,240 bytes (truncated with line count summary if exceeded)
