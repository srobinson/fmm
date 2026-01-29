# exp15: Instruction Delivery Mechanism Comparison

**Date:** 2026-01-29
**Premise:** exp13 proved manifest JSON + explicit instructions = 88-97% token reduction.
This experiment isolates which *delivery mechanism* for those instructions works best.

---

## Conditions

| # | Condition | CLAUDE.md | Skill | MCP Server | What's Being Tested |
|---|-----------|-----------|-------|------------|---------------------|
| A | CLAUDE.md only | Yes | No | No | exp13 baseline — explicit instructions in project config |
| B | Skill only | No | Yes | No | Does a skill achieve the same effect as CLAUDE.md? |
| C | MCP only | No | No | Yes | Does tool availability alone trigger manifest use? |
| D | Skill + MCP | No | Yes | Yes | Is combined integration strictly better? |

### Condition Details

**A — CLAUDE.md only:**
- `.fmm/index.json` exists
- `CLAUDE.md` contains fmm navigation instructions (from `docs/CLAUDE-SNIPPET.md`)
- No skill installed, no MCP server running
- Claude reads CLAUDE.md at session start, then follows instructions

**B — Skill only:**
- `.fmm/index.json` exists
- `.claude/skills/fmm-navigate.md` installed (via `fmm init --skill`)
- No CLAUDE.md fmm content, no MCP server
- Claude loads skill automatically, reads manifest via Read tool

**C — MCP only:**
- `.fmm/index.json` exists
- MCP server configured in `.mcp.json` (via `fmm init --mcp`)
- No CLAUDE.md fmm content, no skill installed
- Tests: Does Claude discover and use fmm tools without being told to?

**D — Skill + MCP:**
- `.fmm/index.json` exists
- Both skill and MCP server active
- Skill tells Claude about MCP tools; MCP provides structured queries
- Expected: Best experience — skill provides context, MCP provides efficiency

---

## Test Tasks

Reuse exp13 task categories with standardized prompts:

1. **Architecture exploration:** "Describe the architecture of this project. What are the main modules and how do they interact?"
2. **Export lookup:** "Find where the function `{name}` is defined and what module it belongs to."
3. **Impact analysis:** "If I change the function signature of `{name}`, what files would be affected?"
4. **Dependency mapping:** "What external packages does this project depend on? List the top 10 by usage."

---

## Metrics

Per run:
- `tool_calls`: Total tool invocations
- `read_calls`: Read tool specifically
- `mcp_calls`: MCP tool calls (conditions C, D only)
- `files_accessed`: Unique files read
- `lines_read`: Total lines of source code consumed
- `manifest_accessed`: Boolean — did the agent read .fmm/index.json?
- `input_tokens`: Total input tokens
- `output_tokens`: Total output tokens
- `accuracy`: Correctness score (0-5, manually graded)
- `duration_ms`: Wall clock time

---

## Execution

### Setup per condition:

```bash
# Clone target repo into sandbox
git clone <target-repo> /tmp/exp15-sandbox

# Generate manifest
cd /tmp/exp15-sandbox && fmm generate --manifest-only

# Condition A: Add CLAUDE.md snippet
cat docs/CLAUDE-SNIPPET.md >> /tmp/exp15-sandbox/CLAUDE.md

# Condition B: Install skill
cd /tmp/exp15-sandbox && fmm init --skill

# Condition C: Install MCP config
cd /tmp/exp15-sandbox && fmm init --mcp

# Condition D: Install both
cd /tmp/exp15-sandbox && fmm init --skill && fmm init --mcp
```

### Run per condition:

```bash
# 3 runs per condition, per task = 48 total runs
claude --output-format stream-json \
  --allowedTools "Read,Glob,Grep,mcp__fmm__*" \
  -p "$TASK_PROMPT" \
  2>&1 | tee run-${condition}-${task}-${run}.json
```

### Parse results:

```bash
# Extract metrics from stream JSON
fmm compare --parse-stream run-*.json --output exp15-results.json
```

---

## Target Repository

Use the same codebase as exp13: a real-world project with 100+ files.
Ideally one where `fmm generate` produces a rich manifest with many exports, imports, and dependencies.

Candidate: The fmm project itself (self-referential but rich metadata).

---

## Hypotheses

| Hypothesis | Prediction | Rationale |
|------------|------------|-----------|
| H1: Skill ≈ CLAUDE.md | B achieves within 10% of A's metrics | Skills and CLAUDE.md deliver instructions identically — both are loaded at session start |
| H2: MCP alone is insufficient | C shows significantly less manifest usage than A/B | Without explicit instructions, Claude won't discover/use fmm tools spontaneously |
| H3: Skill + MCP is strictly best | D outperforms all others on efficiency | Skill provides context (when to use tools), MCP provides capability (structured queries) |
| H4: MCP enables better dep queries | D outperforms B on impact analysis tasks | `fmm_dependency_graph` gives structured results vs. manual manifest parsing |

---

## Success Criteria

1. 3 runs per condition minimum (12 total, ideally 48 with 4 tasks)
2. Metrics captured for all runs
3. Clear statistical comparison across conditions
4. Recommendation: which mechanism(s) should `fmm init` install by default?
