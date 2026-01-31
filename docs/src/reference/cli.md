# CLI Reference

<!-- This file is auto-generated. Do not edit manually. -->
<!-- Run `just generate-cli-docs` to regenerate. -->

## Usage

```
fmm <COMMAND>
```

## Core Commands

### `fmm generate`

Create `.fmm` sidecar files for source files that don't already have them.

```
fmm generate [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` — Path to file or directory (default: `.`)

**Options:**
- `-n, --dry-run` — Show what would be created without writing files

**Examples:**
```bash
fmm generate              # All files in current directory
fmm generate src/         # Specific directory
fmm generate -n           # Preview without writing
```

### `fmm update`

Regenerate all `.fmm` sidecars from source. Unlike `generate`, this overwrites existing sidecars.

```
fmm update [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` — Path to file or directory (default: `.`)

**Options:**
- `-n, --dry-run` — Show what would be changed without writing files

### `fmm validate`

Check sidecars are up to date. Returns exit code 0 if current, 1 if stale. Designed for CI pipelines.

```
fmm validate [PATH]
```

**Arguments:**
- `[PATH]` — Path to file or directory (default: `.`)

### `fmm clean`

Remove all `.fmm` sidecar files and the legacy `.fmm/` directory.

```
fmm clean [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` — Path to file or directory (default: `.`)

**Options:**
- `-n, --dry-run` — Show what would be removed without deleting

## Setup

### `fmm init`

Set up fmm in the current project. Creates config, installs Claude Code skill, and configures MCP server.

```
fmm init [OPTIONS]
```

**Options:**
- `--skill` — Install Claude Code skill only
- `--mcp` — Install MCP server config only
- `--all` — Install all integrations
- `--no-generate` — Skip auto-generating sidecars

### `fmm status`

Display current fmm configuration, supported languages, and workspace statistics.

```
fmm status
```

## Integration

### `fmm mcp`

Start the Model Context Protocol (MCP) server over stdio. Exposes fmm's search and metadata capabilities as tools for LLM agents.

```
fmm mcp
```

### `fmm gh issue`

Fix a GitHub issue automatically: clone repo, generate sidecars, invoke Claude with focused context, create PR.

```
fmm gh issue <URL> [OPTIONS]
```

**Arguments:**
- `<URL>` — GitHub issue URL

**Options:**
- `--model <MODEL>` — Claude model (default: `sonnet`)
- `--max-turns <N>` — Maximum turns (default: `30`)
- `--max-budget <USD>` — Maximum budget (default: `5.0`)
- `-n, --dry-run` — Show plan without executing
- `--branch-prefix <PREFIX>` — Git branch prefix (default: `fmm`)
- `--no-pr` — Commit and push only, skip PR creation
- `--workspace <DIR>` — Override workspace directory

## Analysis

### `fmm search`

Query sidecar metadata by export, import, dependency, or line count.

```
fmm search [OPTIONS]
```

**Options:**
- `-e, --export <NAME>` — Find file by export name (O(1) lookup)
- `-i, --imports <MODULE>` — Find files that import a module
- `-l, --loc <EXPR>` — Filter by line count (`>500`, `<100`, `=200`)
- `-d, --depends-on <PATH>` — Find files that depend on a path
- `-j, --json` — Output as JSON

**Examples:**
```bash
fmm search --export createStore     # Find symbol definition
fmm search --imports react          # Find React consumers
fmm search --loc ">500"             # Find large files
fmm search --depends-on src/db.ts   # Find dependents
fmm search --json                   # Machine-readable output
```

### `fmm compare`

Benchmark FMM-assisted vs unassisted Claude performance on a GitHub repository.

```
fmm compare <URL> [OPTIONS]
```

**Arguments:**
- `<URL>` — GitHub repository URL

**Options:**
- `-b, --branch <BRANCH>` — Branch to compare
- `--src-path <PATH>` — Path within repo to analyze
- `--tasks <SET>` — Task set: `standard`, `quick`, or custom JSON path
- `--runs <N>` — Runs per task (default: `1`)
- `-o, --output <DIR>` — Output directory
- `--format <FMT>` — Output format: `json`, `markdown`, `both`
- `--max-budget <USD>` — Maximum budget (default: `10.0`)
- `--no-cache` — Skip cache
- `--quick` — Quick mode
- `--model <MODEL>` — Model to use (default: `sonnet`)
