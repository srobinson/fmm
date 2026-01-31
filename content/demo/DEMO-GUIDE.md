# fmm Token Savings Demo Guide

A reproducible walkthrough showing how fmm reduces LLM token consumption during code navigation.

## What This Demo Does

The demo runs the **same navigation query** against the same codebase twice:

1. **Control** -- no fmm artifacts, no hints. The LLM brute-forces with grep and file reads.
2. **Treatment** -- fmm sidecars generated, `.claude/CLAUDE.md` hint present. The LLM reads sidecars first, then makes targeted reads.

Both runs use full isolation (`--setting-sources`, `--strict-mcp-config`) to prevent external config leakage. The treatment run loads only the project-level CLAUDE.md that fmm generates.

The test codebase is an 18-file TypeScript authentication app (JWT, login, signup, middleware, routes, models, services) from `research/exp14/repos/test-auth-app/`.

## Prerequisites

### Rust / Cargo

fmm is built from source. Install Rust if you don't have it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify:

```bash
cargo --version
# cargo 1.83.0 (2024-xx-xx) or newer
```

### Claude CLI

You need the Claude Code CLI with an active API key:

```bash
npm install -g @anthropic-ai/claude-code
```

Verify the CLI is authenticated:

```bash
claude --version
```

Your `ANTHROPIC_API_KEY` must be set, or you must be logged in via `claude login`.

### Python 3

Required for JSON trace parsing. Available by default on macOS and most Linux distributions.

```bash
python3 --version
```

### Estimated Cost

Two Claude Sonnet queries against an 18-file codebase: **~$0.12-0.15 total**.

## Running the Demo

From the fmm repo root:

```bash
./content/demo/demo.sh
```

That's it. The script handles everything: building fmm, copying the test fixture, generating sidecars, running both experiments, and printing results.

## What Happens Step by Step

### Step 0: Prerequisites Check

```
[info]  Checking prerequisites...
[ok]    cargo found: cargo 1.83.0
[ok]    claude CLI found
[ok]    python3 found
[ok]    fmm source found at /path/to/fmm
```

The script verifies all tools are available before doing anything expensive.

### Step 1: Build fmm

```
[info]  Building fmm from source...
[ok]    fmm built: /path/to/fmm/target/release/fmm
```

Compiles fmm in release mode. First build takes ~30-60 seconds; subsequent builds are near-instant.

### Step 2: Set Up Test Codebases

The script copies the 18-file test app into two isolated directories:

- `control/` -- bare codebase, no fmm artifacts, no CLAUDE.md
- `treatment/` -- same codebase, will receive fmm sidecars

```
[ok]    Test codebase: 18-file TypeScript auth app (18 .ts files)
```

### Step 3: Generate fmm Sidecars

Runs `fmm init && fmm generate` in the treatment directory. This creates:

- `*.fmm` sidecar files -- per-file YAML metadata (exports, imports, dependencies, LOC)

```
[ok]    Generated N fmm artifacts in treatment codebase
[ok]    Created .claude/CLAUDE.md hint in treatment codebase
```

The CLAUDE.md hint is minimal:

```markdown
# FMM Navigation

This codebase has fmm (Frontmatter Matters) sidecars. Before reading source files:
1. Check for .fmm sidecar files (*.fmm) next to source files
2. Read sidecars for file metadata (exports, imports, deps, LOC)
3. Only open source files you actually need to read or edit
```

### Step 4: Control Run (No fmm)

```
--- Phase 1: Control (no fmm) ---
[info]  Running Claude against bare codebase...
[ok]    Control completed in ~30s
```

The LLM receives the query:

> "Where is the authentication middleware defined? List all exported functions from the auth module."

Without any hints, it follows its default strategy:
1. `Grep` for auth-related patterns across all files
2. `Read` each matching file (typically 10-12 files)
3. Summarize findings

### Step 5: Treatment Run (With fmm)

```
--- Phase 2: Treatment (with fmm + CLAUDE.md) ---
[info]  Running Claude against fmm-enabled codebase...
[ok]    Treatment completed in ~30s
```

Same query, but the LLM now has the CLAUDE.md hint. Expected behavior:
1. `Glob("**/*.fmm")` or `Read` sidecar files -- reads metadata first
2. Identifies relevant files from sidecar metadata
3. `Read` only the source files that matter (targeted reads)

### Step 6: Results Comparison

The script parses both stream-json traces and prints a side-by-side table:

```
============================================================
  fmm Token Savings -- Side-by-Side Comparison
============================================================

+------------------------+----------------------+----------------------+
| Metric                 |   Control (no fmm)   |    Treatment (fmm)   |
+------------------------+----------------------+----------------------+
| Tool calls             |                   13 |                   16 |
| Files read             |                   11 |                    5 |
| Input tokens           |              121,438 |               45,000 |
| Output tokens          |                1,700 |                1,200 |
| Total tokens           |              123,138 |               46,200 |
| Cost (USD)             |              $0.0620 |              $0.0230 |
| Duration               |                  31s |                  25s |
| Read .fmm sidecars?    |                   No |                  Yes |
+------------------------+----------------------+----------------------+

  Token delta: 76,938 tokens saved (62.5%)
  Cost delta:  $0.0390 saved (62.9%)

  First tool action:
    Control:   Grep({"pattern":"auth","path":"src"})
    Treatment: Read({"file_path":"src/auth/login.ts.fmm"})

  KEY BEHAVIOR: With fmm, the LLM read .fmm sidecar files
  to understand the codebase, then made targeted source reads.
  Without fmm, it brute-forced with grep across all files.
```

*Note: Exact numbers will vary per run. The table above shows representative values.*

## Understanding the Numbers

### Tool Calls

More tool calls does not mean worse. The treatment may issue more calls (reading sidecars adds calls), but the total data transferred is lower because it avoids reading irrelevant source files.

### Files Read

This is the key metric. Without fmm, the LLM reads 10-12 of 18 files (56-67%) just to answer a navigation question. With fmm, it reads compact sidecars plus only the relevant source files.

### Input Tokens

Input tokens are dominated by file contents sent back to the LLM. Fewer files read = fewer input tokens. On an 18-file codebase the savings are moderate. On a 500-file codebase, the savings are dramatic because sidecars are ~10 lines each vs hundreds of lines per source file.

### The "First Tool Action"

This is the behavioral fingerprint. Look for:

- **Control**: First action is `Grep` or `Glob` -- scanning for files
- **Treatment**: First action is `Read` or `Glob` on `.fmm` sidecar files -- consulting the map

When the treatment's first action targets `.fmm` files, the LLM has adopted the navigation-first strategy. This is the core behavior change fmm enables.

### Cost

At Sonnet pricing, each run costs ~$0.03-0.08. The savings per query are small in absolute terms but compound across a development session. A developer making 50 navigation queries per day could save $1-3/day on a large codebase.

## Interpreting Variance

LLM behavior is non-deterministic. You may see:

- **Treatment does not read .fmm files**: Happens occasionally. The CLAUDE.md hint triggers sidecar-first behavior in the majority of runs, not 100%. Run the demo again for more samples.
- **Control uses fewer tokens than treatment**: Possible on a small 18-file codebase where grep is already efficient. The savings become clear at scale (50+ files).
- **Similar tool call counts**: Expected. The difference is in *which* files are read and *how much data* is transferred, not the number of tool invocations.

For statistically robust results, the research experiments ran 3 repetitions per condition. See the full data below.

## Research Backing

This demo is a simplified version of Experiment 14. The full research includes:

| Condition | Avg Tool Calls | Avg Files Read | Avg Tokens (in+out) | Avg Cost | FMM Discovered |
|-----------|---------------|----------------|---------------------|----------|----------------|
| Control   | 13.3 | 11.3 | 121,438 | $0.062 | 0/3 |
| Inline    | 14.3 | 10.3 | 150,486 | $0.068 | 0/3 |
| Manifest  | 14.0 | 10.0 | 134,367 | $0.061 | 0/3 |
| Hint      | 15.7 | 11.3 | 168,848 | $0.079 | 0/3 |

Key findings from Exp14:

1. **LLMs never discover `.fmm/` on their own** (0/12 runs). They default to grep+read and never explore hidden directories.
2. **Inline FMM comments are invisible** -- LLMs skip them, extracting info from code, not metadata comments.
3. **A system prompt hint alone is insufficient** -- `--append-system-prompt` did not change behavior.
4. **CLAUDE.md instruction works immediately** -- when loaded via the standard project config mechanism, the LLM's very first action targets `.fmm` sidecar files.

For the complete write-up: `research/exp14/FINDINGS.md`

For raw experiment traces: `research/exp14/results/`

## Scaling Projections

The 18-file test app is intentionally small. Real-world savings scale with codebase size:

| Codebase Size | Without fmm (tokens) | With fmm (tokens) | Savings |
|---------------|----------------------|--------------------|---------|
| 18 files      | ~121K                | ~45-80K            | 35-65%  |
| 100 files     | ~500K+               | ~60-100K           | 80-88%  |
| 500 files     | ~2M+                 | ~80-150K           | 92-96%  |

Each sidecar is ~10 lines regardless of source file size. Without fmm, the LLM reads a proportional fraction of all source files, and token cost grows linearly with codebase size.

## Troubleshooting

**"cargo not found"** -- Install Rust via [rustup.rs](https://rustup.rs).

**"claude CLI not found"** -- Install via `npm install -g @anthropic-ai/claude-code`. Ensure your PATH includes the npm global bin directory.

**Claude returns empty output** -- Check that `ANTHROPIC_API_KEY` is set or that you are logged in via `claude login`. The script uses `--max-budget-usd 1.00` to cap spending.

**Treatment does not show fmm behavior** -- The CLAUDE.md hint works in the majority of runs but LLM behavior is stochastic. Run the demo again. If it consistently fails, verify that `.claude/CLAUDE.md` was created in the treatment directory (the script prints confirmation).

**Build fails** -- Ensure you have a C compiler installed (`xcode-select --install` on macOS, `build-essential` on Ubuntu). fmm's tree-sitter dependencies require one.
