# Benchmark Data

## Test Environment

- **Codebase:** agentic-flow (244 TypeScript files, 81,732 lines)
- **Model:** Claude Opus 4.5
- **Date:** 2026-01-28

---

## Test 0: Review Recent Changes

**Task:** "Review what changed this week and summarize"

| Metric | Control | FMM |
|--------|---------|-----|
| Read tool calls | 10 | 3 |
| Lines read | 1,824 | 65 |
| Reduction | - | **96.4%** |

---

## Test 1: Refactor Impact Analysis

**Task:** "Analyze adding optional configPath parameter to loadConfig"

| Metric | Control | FMM |
|--------|---------|-----|
| Read tool calls | 17 | 23 |
| Lines read | 2,800 | 345 |
| Files identified | 14 | 20+ |
| Reduction | - | **87.7%** |

Note: FMM made more calls but read far fewer lines (peek strategy).

---

## Test 2: Security Review (Small Codebase)

**Task:** Find bugs in 4-file test codebase (6 planted vulnerabilities)

| Metric | Control | FMM |
|--------|---------|-----|
| Read tool calls | 4 | 7 |
| Lines read | 123 | 120 |
| Bugs found | 6/6 | 6/6 |
| Bonus findings | 2 | 3 |
| Reduction | - | **~0%** |

Note: Tiny codebase = frontmatter overhead offsets savings.

---

## Test 3: Architecture Exploration

**Task:** "Understand how the swarm system works"

| Metric | Control | FMM |
|--------|---------|-----|
| Files analyzed | 12 | 12 |
| Read tool calls | 12 | 12 |
| Full file reads | 12 | 0 |
| Frontmatter-only reads | 0 | 12 |
| Lines read | 7,135 | 180 |
| Reduction | - | **97.5%** |

---

## Summary

| Test | Task Type | Lines (Control) | Lines (FMM) | Reduction |
|------|-----------|-----------------|-------------|-----------|
| 0 | Review changes | 1,824 | 65 | 96.4% |
| 1 | Refactor analysis | 2,800 | 345 | 87.7% |
| 2 | Bug finding (tiny) | 123 | 120 | ~0% |
| 3 | Architecture explore | 7,135 | 180 | 97.5% |

**Average reduction on real codebases: ~94%**

---

## Crossover Analysis

FMM wins when: `files_skipped × avg_file_size > frontmatter_overhead`

| Codebase Size | Avg File LOC | Files to Skip | Break-Even |
|---------------|--------------|---------------|------------|
| 4 files | 30 | 1+ | Barely |
| 50 files | 200 | 2+ | Easy win |
| 244 files | 335 | 5+ | Massive win |

Real codebases are large. FMM wins by default.
