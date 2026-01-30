# FMM CLI Reference

## Overview

**fmm** (Frontmatter Matters) is a CLI tool that auto-generates code metadata sidecars for LLM-optimized code navigation. It dramatically reduces token costs (88-97% savings) by allowing LLMs to query structured metadata instead of reading entire source files.

**Version:** 0.1.0
**Author:** Stuart Robinson
**License:** MIT
**Repository:** https://github.com/mdcontext/fmm

---

## Core Concepts

### Sidecar Files (.fmm)
- **Format:** One `.fmm` YAML-like sidecar per source file (e.g., `src/auth.ts` → `src/auth.ts.fmm`)
- **Location:** Adjacent to source files
- **Contents:** Structured metadata (exports, imports, dependencies, LOC, language-specific fields)
- **Primary Purpose:** Enable LLM queries without reading source code
- **Human Benefit:** At-a-glance file structure understanding

### Manifest
- **Purpose:** In-memory index built from all `.fmm` sidecars
- **Format:** JSON with version, timestamp, file entries, and export index
- **Scope:** Loaded on-demand by MCP server and CLI
- **Export Index:** O(1) symbol-to-file lookup table

### Integration Points
1. **Config File:** `.fmmrc.json` in project root
2. **Skill:** `.claude/skills/fmm-navigate.md` for Claude Code
3. **MCP:** `.mcp.json` configuration for LLM servers
4. **Ignore Patterns:** `.fmmignore` for excluding files (respects `.gitignore`)

---

## Configuration File Format (.fmmrc.json)

### Full Schema

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024
}
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `languages` | array | `["ts", "tsx", "js", "jsx", "py", "rs", "go"]` | File extensions to process. Supported: `ts`, `tsx`, `js`, `jsx`, `py`, `rs`, `go`, `java`, `cpp`, `hpp`, `cc`, `hh`, `cxx`, `hxx`, `cs`, `rb` |
| `format` | string | `"yaml"` | Sidecar format. Options: `yaml`, `json` |
| `include_loc` | boolean | `true` | Include line-of-code counts in sidecars |
| `include_complexity` | boolean | `false` | Include cyclomatic complexity metrics (experimental) |
| `max_file_size` | number | `1024` | Maximum file size to process, in KB (1024 = 1MB). Files larger than this are skipped |

### Defaults
- If `.fmmrc.json` doesn't exist, defaults are used
- `init` command creates `.fmmrc.json` with defaults
- Config is loaded lazily by each command; changes take effect immediately

---

## Sidecar File Format

### YAML Structure (Default)

```yaml
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis-client]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-27
rust:
  derives: [Debug, Clone]
  unsafe_blocks: 1
```

### Fields

| Field | Type | Always Present | Description |
|-------|------|----------------|-------------|
| `file` | string | Yes | Relative path to source file (first field for LLM orientation) |
| `fmm` | string | No | Version string (e.g., `v0.2`). Only written if explicitly set |
| `exports` | array | Conditional | Top-level exports (functions, classes, types, variables) by language parsing rules |
| `imports` | array | Conditional | External package imports/dependencies |
| `dependencies` | array | Conditional | Local relative imports (intra-project dependencies) |
| `loc` | number | Yes | Non-comment, non-blank lines of code |
| `modified` | string | Yes | Last generation date in ISO format (YYYY-MM-DD) |
| `<language>` | object | Conditional | Language-specific custom fields (see below) |

### Language-Specific Custom Fields

#### TypeScript/JavaScript
- No custom fields (extracts functions, classes, interfaces, variables)

#### Python
- `decorators`: List of decorators used (e.g., `@property`, `@staticmethod`, `@app.route`)

#### Rust
- `derives`: Derive macros (e.g., `Debug`, `Clone`, `Serialize`)
- `unsafe_blocks`: Count of `unsafe` blocks
- `trait_impls`: Trait implementations (e.g., `Display for Error`)
- `lifetimes`: Lifetime parameters used (e.g., `'a`, `'static`)
- `async_functions`: Count of `async fn` declarations

#### Java
- `annotations`: Annotations used (e.g., `@Service`, `@Override`)

#### C++
- `namespaces`: Namespace definitions and usages

#### C#
- `namespaces`: Namespace declarations
- `attributes`: Attributes applied (e.g., `[Serializable]`)

#### Go
- No custom fields (Capitalized exports detected automatically)

#### Ruby
- `mixins`: Included/extended/prepended modules

---

## CLI Commands

### 1. `fmm generate [PATH] [OPTIONS]`

**Purpose:** Create `.fmm` sidecar files for source files that don't have them

**Arguments:**
- `[PATH]` (default: `.`) — File or directory to process

**Options:**
- `-n, --dry-run` — Show what would be changed without writing files

**Behavior:**
- Scans directory recursively, respecting `.gitignore` and `.fmmignore`
- Filters files by language extensions in config
- **Skips** files that already have sidecars (doesn't overwrite)
- Processes files in parallel using all CPU cores
- Returns summary: count of sidecars written, or message if all up-to-date

**Exit Code:** 0 on success

**Example:**
```bash
fmm generate src/
fmm generate src/ -n  # Dry run
```

---

### 2. `fmm update [PATH] [OPTIONS]`

**Purpose:** Regenerate/update all `.fmm` sidecar files from current source

**Arguments:**
- `[PATH]` (default: `.`) — File or directory to process

**Options:**
- `-n, --dry-run` — Show what would be changed without writing files

**Behavior:**
- Scans all supported files in directory
- **Always** regenerates sidecars (overwrites existing)
- Only writes sidecars if content changed (smart detection)
- Parallel processing, respects `.gitignore` and `.fmmignore`
- Reports count of updated sidecars

**Use Cases:**
- Refresh metadata after code changes
- Sync sidecars with latest source
- Part of pre-commit hooks to keep manifest in sync

**Example:**
```bash
fmm update src/
fmm update  # Update current directory and all subdirs
```

---

### 3. `fmm validate [PATH]`

**Purpose:** Check that `.fmm` sidecars are up-to-date with source files

**Arguments:**
- `[PATH]` (default: `.`) — File or directory to validate

**Options:** None

**Behavior:**
- Compares each source file's current exports/imports/dependencies/loc against sidecar
- Reports files with missing or outdated sidecars
- Parallel processing
- **Exit code 1 if validation fails** (useful for CI)

**Output on Success:**
```
✓ All sidecars are up to date!
```

**Output on Failure:**
```
✗ 2 files need updating:
  ✗ src/auth.ts: sidecar out of date
  ✗ src/utils.ts: missing sidecar
```

**Use Cases:**
- CI/CD pipelines to enforce manifest freshness
- Pre-commit validation before merging
- Development workflow checks

**Example:**
```bash
fmm validate src/
fmm validate  # Validate entire project
```

---

### 4. `fmm clean [PATH] [OPTIONS]`

**Purpose:** Remove all `.fmm` sidecar files and legacy `.fmm/` directory

**Arguments:**
- `[PATH]` (default: `.`) — File or directory to clean

**Options:**
- `-n, --dry-run` — Show what would be removed without deleting

**Behavior:**
- Finds and removes all `.fmm` sidecar files
- Also removes legacy `.fmm/` directory (if present from earlier versions)
- Reports count of removed files
- **Destructive operation** — use dry-run first to verify

**Example:**
```bash
fmm clean src/ -n  # Preview what will be removed
fmm clean src/     # Actually remove sidecars
```

---

### 5. `fmm init [OPTIONS]`

**Purpose:** Initialize fmm in a project (interactive or non-interactive)

**Options:**
- `--skill` — Install Claude Code skill only (`.claude/skills/fmm-navigate.md`)
- `--mcp` — Install MCP server config only (`.mcp.json`)
- `--all` — Install all integrations at once (non-interactive)
- **No flag** — Interactive mode (prompts for each component)

**Behavior:**
- **Default mode (no flags):** Prompts user for each integration
- **`--all` flag:** Installs all integrations without prompts
- **`--skill` only:** Creates/updates only the Claude skill
- **`--mcp` only:** Creates/updates only the MCP config
- Skips existing files if content matches (shows yellow `!` message)

**Creates/Updates:**
1. `.fmmrc.json` — Default configuration
2. `.claude/skills/fmm-navigate.md` — Claude Code skill (if skill option selected)
3. `.mcp.json` — MCP server configuration (if mcp option selected)

**Output Example:**
```
Initializing fmm...
✓ Created .fmmrc.json with default configuration
✓ Installed Claude skill at .claude/skills/fmm-navigate.md
✓ Created .mcp.json with fmm server configuration

Setup complete!
  Skill:    .claude/skills/fmm-navigate.md
  MCP:      .mcp.json

Run `fmm generate` to create sidecar files.
```

**Usage Pattern:**
```bash
fmm init --all               # Full setup in one command
fmm init --skill             # Add skill to existing project
fmm init --mcp               # Add MCP to existing project
```

---

### 6. `fmm status`

**Purpose:** Display current fmm configuration and workspace state

**Arguments:** None
**Options:** None

**Output:**
```
fmm Status
========================================

Configuration:
  ✓ .fmmrc.json found

Settings:
  Format:         YAML
  Include LOC:    yes
  Max file size:  1024 KB

Supported Languages:
  go, js, jsx, py, rs, ts, tsx

Workspace:
  Path: /Users/alphab/Dev/LLM/DEV/fmm
  42 source files, 25 sidecars
```

**Use Cases:**
- Verify configuration is loaded correctly
- Check workspace scan results
- Quick status check during development

---

### 7. `fmm search [OPTIONS]`

**Purpose:** Query the manifest for files and exports

**Options:**
- `-e, --export <EXPORT>` — Find file by exact export name
- `-i, --imports <IMPORTS>` — Find files importing a package (substring match)
- `-l, --loc <LOC>` — Filter by line count (expressions: `>500`, `<100`, `=200`, `>=50`, `<=1000`)
- `-d, --depends-on <PATH>` — Find files depending on a local path
- `-j, --json` — Output results as JSON (pretty-printed)

**Behavior:**
- Builds manifest from all `.fmm` sidecars in current directory
- Filters combine with AND logic (all conditions must match)
- **LOC expressions:** Supports comparison operators; plain number defaults to `=`
- If no filters provided, lists all files
- Substring matching for imports/dependencies; exact matching for exports

**Output (Text):**
```
✓ 2 file(s) found:

src/auth/session.ts
  exports: createSession, validateSession
  imports: jwt, redis
  loc: 234

src/api/middleware.ts
  exports: authMiddleware
  imports: express
  loc: 89
```

**Output (JSON):**
```json
[
  {
    "file": "src/auth/session.ts",
    "exports": ["createSession", "validateSession"],
    "imports": ["jwt", "redis"],
    "dependencies": ["./types", "./config"],
    "loc": 234
  }
]
```

**Examples:**
```bash
fmm search --export validateUser              # Find where validateUser is defined
fmm search --imports crypto                   # Find files using crypto
fmm search --loc ">500"                       # Find large files (>500 LOC)
fmm search --depends-on ./types               # Find files depending on ./types
fmm search --imports express --loc ">=100"    # Files importing express with 100+ LOC
fmm search --json                             # List all files as JSON
```

---

### 8. `fmm mcp` / `fmm serve`

**Purpose:** Start MCP (Model Context Protocol) server for LLM integration

**Arguments:** None
**Options:** None

**Behavior:**
- Loads manifest from sidecars on startup
- Listens on stdin/stdout for JSON-RPC 2.0 requests
- Responds with structured tool capabilities
- Auto-reloads manifest on `tools/call` requests
- Implements 5 core tools (see below)

**MCP Tools Available:**

#### `fmm_lookup_export(name: string)`
- **Purpose:** O(1) symbol-to-file lookup
- **Returns:** File path + full metadata (exports, imports, dependencies, LOC)
- **Use:** "Where is this function/class defined?"
- **Example:** `fmm_lookup_export(name: "createSession")`
- **Response:** File path and all metadata from that file

#### `fmm_list_exports(pattern?: string, file?: string)`
- **Purpose:** Search or list exported symbols
- **Options:**
  - `pattern` — Substring match across all exports (case-insensitive)
  - `file` — List all exports from a specific file
  - Neither — List all exports grouped by file
- **Examples:**
  - `fmm_list_exports(pattern: "auth")` → all exports containing "auth"
  - `fmm_list_exports(file: "src/api/routes.ts")` → exports from that file

#### `fmm_file_info(file: string)`
- **Purpose:** Get a file's structural profile
- **Returns:** Exports, imports, dependencies, LOC
- **Equivalent To:** Reading the file's `.fmm` sidecar
- **Use:** "Tell me about this file without reading it"

#### `fmm_dependency_graph(file: string)`
- **Purpose:** Analyze file dependencies and dependents
- **Returns:**
  - `upstream`: Files this file depends on (its dependencies)
  - `downstream`: Files that depend on this file
  - `imports`: External package imports
- **Use:** Impact analysis, blast radius assessment
- **Example:** "What breaks if I change src/auth.ts?"

#### `fmm_search(export?, imports?, depends_on?, min_loc?, max_loc?)`
- **Purpose:** Multi-criteria search with AND logic
- **Parameters:**
  - `export` — Exact export name
  - `imports` — Package/module substring
  - `depends_on` — Local dependency path substring
  - `min_loc` — Minimum lines of code
  - `max_loc` — Maximum lines of code
- **Examples:**
  - Find all files under 100 LOC: `fmm_search(max_loc: 100)`
  - Files importing crypto and >500 LOC: `fmm_search(imports: "crypto", min_loc: 500)`
  - Files depending on ./types: `fmm_search(depends_on: "./types")`

**Integration:**
```json
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["serve"]
    }
  }
}
```

**Why MCP?**
- **Speed:** O(1) lookups vs. recursive grep
- **Structured:** Tools return JSON for programmatic use
- **Efficient:** Pre-built index, no parsing at query time
- **Claude-Native:** Works with Claude Code and other MCP clients

---

### 9. `fmm compare <URL> [OPTIONS]`

**Purpose:** Benchmark fmm vs control (non-fmm) performance on a GitHub repository

**Arguments:**
- `<URL>` (required) — GitHub repository URL (e.g., `https://github.com/owner/repo`)

**Options:**
- `-b, --branch <BRANCH>` — Branch to compare (default: `main`)
- `--src-path <SRC_PATH>` — Path within repo to analyze (e.g., `src/`)
- `--tasks <TASKS>` — Task set to use (default: `standard`)
  - `standard` — Full benchmark suite
  - `quick` — Fewer tasks for faster results
  - Path to custom JSON file — Custom task definitions
- `--runs <RUNS>` — Number of runs per task (default: `1`)
- `-o, --output <OUTPUT>` — Output directory for results
- `--format <FORMAT>` — Output format (default: `both`)
  - `json` — JSON report only
  - `markdown` — Markdown report only
  - `both` — Both JSON and Markdown
- `--max-budget <MAX_BUDGET>` — Maximum API budget in USD (default: `10.0`)
- `--no-cache` — Skip cache and re-run all tasks
- `--quick` — Quick mode (fewer tasks, faster results)
- `--model <MODEL>` — Model to use (default: `sonnet`)

**Behavior:**
- Clones repository into sandbox
- Generates sidecars (for fmm variant)
- Runs benchmark tasks with Claude CLI
- Compares token usage and time between fmm and control
- Caches results to avoid re-running
- Generates report in JSON and/or Markdown

**Outputs:**
- JSON report with detailed metrics
- Markdown summary with comparison charts
- Token cost analysis
- Performance summary

**Examples:**
```bash
fmm compare https://github.com/vercel/next.js --max-budget 50
fmm compare https://github.com/facebook/react --format markdown
fmm compare https://github.com/microsoft/vscode --quick --runs 2
```

**Use Cases:**
- Validate fmm effectiveness on real projects
- Compare token cost savings
- Benchmark against control group
- Generate reports for documentation

---

## Typical Workflows

### Initial Setup
```bash
# 1. Initialize fmm in your project
fmm init --all

# 2. Generate sidecars for existing code
fmm generate src/

# 3. Verify generation
fmm status
```

### Development Workflow
```bash
# 1. Make code changes
# ... edit source files ...

# 2. Update sidecars
fmm update src/

# 3. Validate before commit
fmm validate src/

# 4. If validation fails, fix code or re-update
fmm update src/
```

### CI/CD Integration
```bash
# In .github/workflows/ci.yml
- name: Check fmm manifest
  run: fmm validate src/

# Or with pre-commit:
# .pre-commit-config.yaml
- repo: local
  hooks:
    - id: fmm-update
      name: Update fmm manifest
      entry: fmm update
      language: system
      pass_filenames: true
```

### Searching and Navigation
```bash
# Find where a function is defined
fmm search --export createSession

# Find files using a package
fmm search --imports crypto

# Find files larger than 500 LOC
fmm search --loc ">500"

# Complex query: files importing express with 100-300 LOC
fmm search --imports express --loc ">=100" --loc "<=300"
```

### Using MCP with Claude
```bash
# 1. Ensure .mcp.json is configured
fmm init --mcp

# 2. Generate sidecars
fmm generate

# 3. Start Claude Code, which connects to fmm MCP server
# 4. Claude can now use fmm_lookup_export, fmm_search, etc.
```

---

## File Filtering

### Ignore Patterns
- **`.fmmignore`** — Custom ignore file (same syntax as `.gitignore`)
- **`.gitignore`** — Automatically respected
- **Supported file extensions** — Controlled by `languages` config

### Example .fmmignore
```
# Ignore test files
**/test/**
**/*.test.ts
**/*.spec.ts

# Ignore vendor
node_modules/
dist/

# Ignore specific files
src/deprecated/**
```

---

## Performance Characteristics

| Operation | Speed | Notes |
|-----------|-------|-------|
| Parse single file | <1ms | TypeScript, Python, Rust |
| Generate 1,000 files | ~670ms | Parallel on all CPU cores |
| Manifest build (from sidecars) | <100ms | Depends on file count |
| Export lookup (MCP) | O(1) | Hash table lookup |
| File search (MCP) | O(n) | Linear scan of manifest |

---

## Error Handling

### Common Issues

**"No sidecars found"**
```
Error: No sidecars found. Run 'fmm generate' first.
```
Solution: Run `fmm generate` to create `.fmm` files

**"Sidecar validation failed"**
```
✗ 2 files need updating:
  ✗ src/auth.ts: sidecar out of date
```
Solution: Run `fmm update` to refresh sidecars

**File skipped due to size**
- Files larger than `max_file_size` (default 1024 KB) are skipped
- Increase `max_file_size` in `.fmmrc.json` if needed

**Language not supported**
- Add extension to `languages` array in `.fmmrc.json`
- Ensure tree-sitter parser exists for the language

---

## Integration Setup Details

### .claude/skills/fmm-navigate.md
- **Purpose:** Guides Claude Code on sidecar-first navigation
- **Content:** Rules for reading sidecars before source, MCP tool usage
- **Auto-installed:** By `fmm init --skill` or `fmm init --all`
- **Location:** `.claude/skills/fmm-navigate.md`

### .mcp.json
- **Purpose:** Configures Claude Code to use fmm MCP server
- **Format:** JSON-RPC 2.0 server config
- **Content:**
```json
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["serve"]
    }
  }
}
```
- **Auto-created:** By `fmm init --mcp` or `fmm init --all`
- **Merging:** If `.mcp.json` exists, fmm config is merged (doesn't overwrite)

---

## Under the Hood

### How Commands Work

1. **`generate` / `update`:**
   - Load `.fmmrc.json` config (or defaults)
   - Scan directory recursively using `ignore` crate (respects `.gitignore`, `.fmmignore`)
   - Filter by file extensions in config
   - Parse each file using tree-sitter (language-specific)
   - Extract metadata (exports, imports, dependencies, LOC, custom fields)
   - Format as YAML/JSON frontmatter
   - Write adjacent `.fmm` sidecar file

2. **`validate`:**
   - Load config and scan files
   - For each source file, regenerate metadata
   - Compare with existing sidecar
   - Report mismatches

3. **`search`:**
   - Load manifest from all `*.fmm` sidecars
   - Build in-memory index with export reverse-lookup table
   - Apply filters (export lookup, import substring, dependency substring, LOC range)
   - Return results

4. **`mcp`:**
   - Load manifest from sidecars
   - Implement JSON-RPC 2.0 protocol
   - Listen on stdin, write to stdout
   - Dispatch tool calls to handlers
   - Auto-reload manifest before tool calls

### Parsing Strategy
- Uses **tree-sitter** for all languages
- Single-pass AST walk to extract:
  - Top-level `export` declarations
  - `import` / `require` statements
  - Relative imports (dependencies)
  - Lines of code (non-comment, non-blank)
  - Language-specific custom fields (decorators, derives, unsafe blocks, etc.)

### Manifest Structure (In-Memory)
```rust
Manifest {
  version: "2.0",
  generated: Utc::now(),
  files: HashMap<file_path, FileEntry>,
  export_index: HashMap<export_name, file_path>,
}

FileEntry {
  exports: [String],
  imports: [String],
  dependencies: [String],
  loc: usize,
}
```

---

## Supported Languages & Coverage

| Language | Extensions | Exports | Imports | Dependencies | Custom Fields | Coverage |
|----------|------------|---------|---------|---------------|---------------|----------|
| TypeScript | .ts, .tsx | ✓ | ✓ | ✓ | — | 20% of GitHub |
| JavaScript | .js, .jsx | ✓ | ✓ | ✓ | — | 25% |
| Python | .py | ✓ | ✓ | ✓ | decorators | 15% |
| Rust | .rs | ✓ | ✓ | ✓ | derives, unsafe_blocks, trait_impls, lifetimes, async_functions | 5% |
| Go | .go | ✓ | ✓ | ✓ | — | 10% |
| Java | .java | ✓ | ✓ | ✓ | annotations | 15% |
| C++ | .cpp, .hpp, .cc, .hh, .cxx, .hxx | ✓ | ✓ | ✓ | namespaces | 10% |
| C# | .cs | ✓ | ✓ | — | namespaces, attributes | 5% |
| Ruby | .rb | ✓ | ✓ | ✓ | mixins | 5% |

**Total coverage:** ~95% of GitHub codebases

---

## Examples

### Example: TypeScript Sidecar
```yaml
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jsonwebtoken, redis]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-28
```

### Example: Python Sidecar
```yaml
file: src/processor.py
fmm: v0.2
exports: [DataProcessor, fetch_data, transform_records]
imports: [pandas, requests, numpy]
dependencies: [.utils, ..models.loader]
loc: 156
modified: 2026-01-28
python:
  decorators: [property, staticmethod, cache]
```

### Example: Rust Sidecar
```yaml
file: src/lib.rs
fmm: v0.2
exports: [Config, Pipeline, process]
imports: [anyhow, serde, tokio]
dependencies: [crate::parser, super::utils]
loc: 280
modified: 2026-01-28
rust:
  derives: [Clone, Debug, Deserialize, Serialize]
  unsafe_blocks: 1
  trait_impls: [Display for Error, Iterator for Pipeline]
  async_functions: 3
  lifetimes: ['a, 'static]
```

---

## Key Implementation Details

### Dependency Resolution
- **Local dependencies** stored as relative paths (e.g., `./types`, `../utils`)
- **MCP tool** `dep_matches()` resolves paths considering:
  - Directory context of dependent file
  - `..` for parent traversal
  - `.` for current directory
  - Extension interchangeability (`.ts` ≈ `.tsx` ≈ `.js` ≈ `.jsx`)

### Export Index
- **O(1) lookup:** Export name → file path
- **Conflict resolution:** TypeScript/React (`.ts`, `.tsx`) preferred over `.js`/`.jsx`
- **Enables:** Fast `fmm_lookup_export` without scanning all files

### Smart Updates
- **Change detection:** Only writes sidecar if content differs
- **Efficient updates:** Parallel processing with rayon crate
- **Preserves structure:** Doesn't modify files unnecessarily

---

This completes the comprehensive CLI reference for fmm. All commands, flags, configuration options, integration setup, and workflows are documented above with examples and use cases.
