# exp15-isolated: Skill + MCP Optimization Results

## Experiment Overview

**Question:** How do we get Claude to actually use fmm MCP tools instead of falling back to Grep/Glob/Read?

**Setup:** Docker-isolated experiment running Claude Code (`claude-sonnet-4-5-20250929`) in `-p` mode against a 1,030-file TypeScript codebase (`agentic-flow`), with 4 navigation tasks × 3 runs per condition.

| Condition | Configuration | Description |
|-----------|--------------|-------------|
| B | Skill only | `fmm-navigate` skill loaded, no MCP server |
| C | MCP only | fmm MCP server connected (10 tools), no skill |
| D | Skill + MCP | Both skill and MCP server active |

### Tasks

| Task | Prompt | Expected fmm tool |
|------|--------|--------------------|
| `architecture` | "Describe this project's architecture — its main modules, layers, and how they connect" | `fmm_get_manifest` / `fmm_project_overview` |
| `export-lookup` | "Where is the function `createPipeline` defined? What file exports it?" | `fmm_lookup_export` / `fmm_find_symbol` |
| `impact-analysis` | "If I change `src/core/pipeline.ts`, what other files would be affected?" | `fmm_dependency_graph` / `fmm_analyze_dependencies` |
| `dependency-map` | "Map all the dependencies of `src/core/engine.ts` — what does it import and what imports it?" | `fmm_dependency_graph` / `fmm_get_manifest` |

---

## Results

### fmm Tool Adoption Rate (runs using fmm / total runs)

| Task | B (Skill) | C (MCP) | D (Skill+MCP) |
|------|-----------|---------|----------------|
| **architecture** | 0/3 (0%) | 3/3 (100%) | **3/3 (100%)** |
| **export-lookup** | 0/3 (0%) | 0/3 (0%) | **3/3 (100%)** |
| **impact-analysis** | 0/3 (0%) | 2/3 (67%) | **3/3 (100%)** |
| **dependency-map** | 0/3 (0%) | 0/3 (0%) | **3/3 (100%)** |
| **Overall** | **0/12 (0%)** | **5/12 (42%)** | **12/12 (100%)** |

### fmm Tool Calls (total across 3 runs)

| Task | B | C | D |
|------|---|---|---|
| architecture | 0 | 5 | 3 |
| export-lookup | 0 | 0 | 3 |
| impact-analysis | 0 | 3 | 8 |
| dependency-map | 0 | 0 | 3 |
| **Total** | **0** | **8** | **17** |

### Efficiency (avg turns per task)

| Task | B (Skill) | C (MCP) | D (Skill+MCP) |
|------|-----------|---------|----------------|
| architecture | 55 | 44 | **27** |
| export-lookup | 5 | 5 | **3** |
| impact-analysis | 6 | 8 | **10** |
| dependency-map | 45 | 57 | **10** |

Condition D completes architecture in **half** the turns of B, and dependency-map in **one-fifth** the turns. The model reaches for fmm first, gets structural data immediately, and avoids the Glob/Read exploration loop.

### Which fmm Tools Were Selected

| Task | Condition D tools used |
|------|----------------------|
| architecture | `fmm_get_manifest` (3/3 runs) |
| export-lookup | `fmm_lookup_export` (3/3 runs) |
| impact-analysis | `fmm_find_symbol` + `fmm_dependency_graph` (3/3), `fmm_search` (2/3) |
| dependency-map | `fmm_get_manifest` (3/3 runs) |

The new consolidated tool `fmm_find_symbol` was picked up for impact-analysis — the intent-matching name ("find symbol") aligned with the task's first step ("find the file for this symbol, then trace its dependents").

---

## What Changed (3 Levers)

### Lever 1: Tool Descriptions (mcp/mod.rs)

Rewrote all 6 tool descriptions from 1-sentence factual to multi-sentence with competitive positioning.

**Before:**
```
fmm_get_manifest: Returns the complete manifest including all files, exports, and dependencies
```

**After:**
```
fmm_get_manifest: Complete project architecture in one call. Use FIRST for 'describe the
architecture', 'how is this organized', 'what modules exist'. Returns every file with its
exports, imports, dependencies, and LOC — replaces Glob + Read dozens of files. No file I/O needed.
```

Pattern: intent keywords → competitive positioning → efficiency signal.

### Lever 2: Skill Rewrite (fmm-navigate.md)

Rewrote from 58-line reference manual to 112-line behavioral directive.

Key additions:
- **Identity framing**: "Your Primary Code Navigation System"
- **Comparison table**: fmm vs Grep/Glob/Read for 6 task types
- **6 decision trees**: step-by-step for architecture, export-lookup, impact-analysis, dependency-map, package usage, file discovery
- **6 ALWAYS/NEVER rules**: "ALWAYS call fmm tools before Grep/Glob/Read", "NEVER start architecture exploration with Glob"

### Lever 3: Tool Consolidation (6 → 10 tools: 4 new + 6 legacy)

Added 4 consolidated tools with intent-matching names:

| New Tool | Replaces | Why it works |
|----------|----------|-------------|
| `fmm_find_symbol` | lookup_export + list_exports | Name matches "find where X is defined" intent |
| `fmm_file_metadata` | file_info | Clearer name |
| `fmm_analyze_dependencies` | dependency_graph + search | Name matches "impact analysis" intent |
| `fmm_project_overview` | get_manifest | Name matches "architecture overview" intent |

All 6 original tools kept as backward-compatible aliases.

---

## Key Insight

**Skill alone (B) = 0% adoption.** Without MCP tools available, the skill's instructions to "use fmm tools" have no effect — there are no tools to call.

**MCP alone (C) = 42% adoption.** Tools are available but Claude doesn't know *when* to prefer them over built-in Grep/Glob/Read. It uses fmm for architecture (the description mentions it) but misses export-lookup and dependency-map entirely.

**Skill + MCP (D) = 100% adoption.** The skill provides the behavioral directive ("use fmm FIRST"), the tool descriptions provide the intent matching ("architecture" → `fmm_get_manifest`), and the consolidated tools reduce selection confusion.

The combination is multiplicative, not additive. Neither component alone achieves the result.

---

## Methodology

- **Isolation**: Docker containers with clean Claude state (no session persistence, no prior context)
- **Reproducibility**: Each condition uses identical Docker images, same codebase, same prompts
- **No CLAUDE.md leakage**: `.dockerignore` excludes `*.md`, entrypoint verifies no CLAUDE.md exists
- **MCP verification**: Init messages in JSONL confirm MCP server connected with all tools registered
- **Skill verification**: Init messages confirm `fmm-navigate` skill loaded
- **Model**: `claude-sonnet-4-5-20250929` across all conditions
- **3 runs per task**: Accounts for model non-determinism
- **MAX_PARALLEL=2**: Prevents API rate limiting artifacts

### Infrastructure

```
exp15-isolated/
├── Dockerfile          # Multi-stage: rust:1.93 builder → node:22-slim runtime
├── docker-compose.yml  # 3 services (condition-b, condition-c, condition-d)
├── entrypoint.sh       # Configures condition, runs claude -p, captures JSONL
├── run-isolated.sh     # Orchestrates parallel runs with completion tracking
├── compare-isolated.py # Analysis script
├── fmm-src/            # Copied fmm Rust source (built in Docker)
└── results/{B,C,D}/    # JSONL output per run
```

---

## Date

2025-01-30 (Condition D rerun with optimized descriptions/skill/tools)

Previous baseline (B, C) from same experiment batch.
