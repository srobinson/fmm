# Navigation Proof — Results Summary

*Generated: 2026-01-30 from live proof runs against 18-file TypeScript auth app*

## Conditions

| | Control | Treatment |
|---|---|---|
| **Setup** | No fmm metadata | `.fmm/index.json` + system prompt hint |
| **Repo** | `research/exp14/repos/clean/` | `research/exp14/repos/hint/` |
| **Codebase** | 18-file TypeScript auth app | Same codebase + fmm manifest |
| **Model** | Claude Sonnet | Claude Sonnet |
| **Isolation** | `--setting-sources ""`, no MCP, no session | Same |

## Per-Query Results

### Query 1: Architecture Overview (best case)

> Describe the architecture of this project. What are the main modules, their roles, key exports, and how they depend on each other? Be specific about file paths.

| Metric | Control | Treatment | Delta |
|--------|---------|-----------|-------|
| Tool calls | 25 | 16 | **-36%** |
| Source files read | 19 | 9 | **-53%** |
| Read calls (total) | 19 | 11 | **-42%** |
| Tokens (total) | 318,589 | 220,895 | **-31%** |
| Duration | 93s | 67s | **-28%** |
| FMM manifest used | No | Yes | |

**Navigation path:**

- **Control:** `ls` -> `Glob(*.ts)` -> Read all 19 source files one by one
- **Treatment:** Read `.fmm/index.json` -> selectively Read 9 key source files

The treatment's **first action** was `Read(.fmm/index.json)`. The manifest gave it a map of all exports and dependencies, so it only opened files it needed to understand deeply.

### Query 2: Export Trace

> Which file defines the createApp function? What does it depend on? Trace the full dependency chain.

| Metric | Control | Treatment | Delta |
|--------|---------|-----------|-------|
| Tool calls | 17 | 18 | +6% |
| Files read | 15 | 16 | +7% |
| Tokens (total) | 227,065 | 241,433 | +6% |
| Duration | 61s | 48s | **-21%** |
| FMM manifest used | No | Yes | |

Both conditions read most files to trace the full dependency chain. On an 18-file codebase, tracing all dependencies means reading nearly everything. The manifest provided the starting point but the chain traversal still required source reads.

### Query 3: Auth Exports

> Find all files that export authentication-related functions. List each file path and the specific auth exports.

| Metric | Control | Treatment | Delta |
|--------|---------|-----------|-------|
| Tool calls | 13 | 16 | +23% |
| Files read | 10 | 13 | +30% |
| Tokens (total) | 114,641 | 188,656 | +65% |
| Duration | 28s | 70s | +150% |
| FMM manifest used | No | Yes | |

The treatment consulted the manifest but then validated by reading source files — actually reading *more* files for completeness. On this small codebase, grep is already efficient for keyword search.

## Aggregate Summary

*Across 3 navigation queries:*

| Metric | Control (total) | Treatment (total) | Delta |
|--------|-----------------|-------------------|-------|
| Tool calls | 55 | 50 | -9% |
| Files read | 44 | 41 | -7% |
| Tokens | 660,295 | 650,984 | -1% |
| Duration | 182s | 185s | +2% |

## Interpretation

The architecture-overview query — "map the whole codebase" — shows the clearest fmm advantage: **36% fewer tool calls, 53% fewer source reads, 31% fewer tokens**. The LLM read the manifest first and skipped files it could characterize from metadata alone.

For targeted queries (export trace, auth search), the deltas are smaller because: (a) the 18-file codebase is small enough that brute-force grep is already fast, and (b) tracing dependencies end-to-end requires reading source regardless. Previous experiments on larger codebases (123-1,306 files) showed **88-97% token reduction** — the benefit compounds with codebase size.

The key behavioral change is consistent: **with fmm, the LLM's first action is always `Read(.fmm/index.json)`**, giving it a structural map before touching source files. Without fmm, it globbed/grepped blindly then read every file it found.

## Key Observation

The manifest acts as a **table of contents**. On a short book (18 files), you might just read every page anyway. On a long book (1,000+ files), the table of contents is what makes navigation possible without reading the whole thing.

| Codebase size | Expected fmm benefit | Evidence |
|---------------|---------------------|----------|
| 18 files | Modest (-36% for architecture tasks) | This proof run |
| 123 files | Strong (-88% to -97% tokens) | `research/exp13` |
| 1,306 files | Very strong (-30% tool calls, -25% cost) | `research/exp15` |
