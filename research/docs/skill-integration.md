# FMM Skill Integration: Complete Documentation

## Overview

FMM (Frontmatter Matters) integrates with Claude Code through a **skill** mechanism that teaches the AI agent to navigate codebases using `.fmm` sidecar files as the primary navigation layer. This document covers the skill architecture, installation, navigation workflow, and performance data from live experiments.

## What the fmm-navigate Skill Does

The skill is located at `/docs/fmm-navigate.md` in the fmm repository and is embedded in the binary via the `include_str!()` macro in the CLI module (`src/cli/mod.rs`, line 427).

The skill instructs Claude Code to:

- **Check sidecars before reading source files** -- avoid unnecessary file reads
- **Use MCP tools when available** for O(1) lookups
- **Fall back to Grep/Glob** only for searching file contents, not structure
- **Understand the dependency graph** for impact analysis

## Installation: `fmm init --skill`

The `init` command in `src/cli/mod.rs` (lines 378-407) orchestrates skill installation:

1. Creates `.claude/skills/` directory if needed
2. Writes the skill content to `.claude/skills/fmm-navigate.md`
3. Idempotent: checks if skill exists and compares content (lines 435-444)
4. Skips installation if already up-to-date

The `init_skill()` function (lines 429-454) handles the actual file write.

### Full Setup: `fmm init --all`

The recommended setup command installs three components:

| Component | File | Purpose |
|-----------|------|---------|
| Config | `.fmmrc.json` | Languages, format, LOC tracking |
| Skill | `.claude/skills/fmm-navigate.md` | Behavioral guidance for Claude |
| MCP | `.mcp.json` | MCP server config with fmm entry |

**Why both Skill and MCP?**
- The skill provides the "why" (when and how to use the manifest)
- MCP provides the "how" (structured queries via tool calls)
- Combined: 30% fewer tool calls, best accuracy, fastest execution

## Navigation Workflow

The skill teaches Claude Code a specific workflow for navigating codebases:

### Step-by-Step Process

1. **Check if `filename.fmm` exists** before opening source files
2. **Use `Grep "exports:.*SymbolName" **/*.fmm`** to find definitions
3. **Read the matching `.fmm` sidecar** for metadata (exports, imports, deps, LOC)
4. **Only open source files you will edit** -- sidecars tell the file's role
5. **Use MCP tools** (if available) for lookups instead of Grep
6. **Fall back to source grep** only for content searches

### Best Practices

1. **CHECK SIDECARS FIRST** before reading source
2. **USE SIDECARS TO NAVIGATE** -- grep sidecars, not source
3. **ONLY OPEN SOURCE FILES YOU WILL EDIT** -- sidecars tell the role
4. **USE MCP TOOLS WHEN AVAILABLE** -- `fmm_lookup_export` is faster than grep
5. **FALL BACK TO GREP/GLOB** only for searching file contents, not structure

## Interaction with Sidecars and Manifest

### Sidecars

Each source file `foo.ts` has a companion `foo.ts.fmm` containing YAML metadata:

```yaml
exports:
  - name: createStore
    type: function
imports:
  - source: "./utils"
    names: [validate]
dependencies:
  - ./utils
loc: 42
```

### Manifest

`Manifest::load_from_sidecars()` (in `src/manifest/mod.rs`) builds an in-memory index at runtime:

- **Export Index:** HashMap mapping every export name to its file path (O(1) lookups)
- **Hot-reload:** The MCP server reloads the manifest on each tool call (`src/mcp/mod.rs`, line 105)

### MCP Tools Available

From `src/mcp/mod.rs` (lines 165-257):

| Tool | Purpose |
|------|---------|
| `fmm_lookup_export(name)` | O(1) symbol-to-file lookup |
| `fmm_list_exports(pattern?, file?)` | Fuzzy search by pattern or list specific file |
| `fmm_file_info(file)` | Metadata without reading source |
| `fmm_dependency_graph(file)` | Upstream deps + downstream dependents |
| `fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})` | Multi-criteria filtering |

## CLAUDE.md Integration

FMM provides a CLAUDE.md snippet in `docs/CLAUDE-SNIPPET.md` that includes instructions about:

- FMM headers and sidecar file format
- Available MCP tools
- Manifest location
- CLI fallbacks

The skill is functionally equivalent to adding instructions to CLAUDE.md -- both achieve approximately the same baseline tool call count (~22.5). Users can add the snippet OR use the skill; they are interchangeable.

## Experiment Results: Skill vs MCP Performance

From `research/exp15/FINDINGS.md` (48 live runs executed across 4 configurations):

### Raw Performance Data

| Metric | A: CLAUDE.md | B: Skill Only | C: MCP Only | D: Skill+MCP |
|--------|-------------|--------------|------------|-------------|
| Avg Tool Calls | 22.2 | 22.5 | 18.2 | **15.5** |
| Avg Reads | 5.2 | 4.1 | 4.6 | **4.8** |
| Cost | $0.55 | $0.47 | $0.50 | **$0.41** |
| Manifest Access | 83% | 75% | 58% | **75%** |
| Duration | 85.8s | 94.5s | 72.2s | **68.5s** |

### Key Findings

1. **Skill equals CLAUDE.md** -- within 1% on tool calls, confirming they are interchangeable
2. **MCP alone is insufficient** -- only 58% manifest access without behavioral guidance
3. **Skill+MCP is strictly best** -- 30% fewer tool calls, 25% cheaper than standalone approaches
4. **Speed improvement** -- Skill+MCP is ~20% faster than CLAUDE.md alone

### Why Skill+MCP Wins

The skill tells Claude *when* and *why* to use the manifest. MCP provides the *mechanism* for structured queries. Without the skill, Claude often ignores available MCP tools (only 58% manifest access in MCP-only mode). Without MCP, Claude must fall back to grep-based navigation which is slower and uses more tool calls.

The combination creates a feedback loop:
- Skill guides Claude to check sidecars first
- MCP makes those checks fast (O(1) lookups)
- Fast checks encourage more frequent use
- More frequent use leads to fewer unnecessary file reads

## Architecture Details

### Skill Embedding

The skill content is embedded in the fmm binary using Rust's `include_str!()` macro:

```rust
// src/cli/mod.rs, line 427
const SKILL_CONTENT: &str = include_str!("../../docs/fmm-navigate.md");
```

This means the skill is always available without external file dependencies.

### Init Command Flow

```
fmm init --all
  ├── init_config()     → .fmmrc.json
  ├── init_skill()      → .claude/skills/fmm-navigate.md
  └── init_mcp()        → .mcp.json (with fmm server entry)
```

Each init function is idempotent -- it checks for existing files and only writes if the content differs or the file is missing.

### Integration Points

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  Claude Code │────>│  fmm Skill   │────>│  .fmm files │
│  (Agent)     │     │  (Behavior)  │     │  (Sidecars)  │
└──────┬───────┘     └──────────────┘     └──────┬───────┘
       │                                         │
       │         ┌──────────────┐                │
       └────────>│  MCP Server  │<───────────────┘
                 │  (Tools)     │
                 └──────────────┘
```

Claude Code reads the skill for navigation guidance, then uses MCP tools to query the manifest (built from sidecar files) for fast, structured lookups.
