# FMM Architecture Decisions

This document records settled architecture decisions with their experimental evidence.
It exists to prevent future reversions to approaches already proven ineffective.

## Settled Decisions

### 1. Sidecar files (`file.ext.fmm`)

Every source file gets a companion `.fmm` sidecar containing structured YAML metadata
(exports, imports, dependencies, LOC). Sidecars are independently addressable — an LLM
reads `foo.ts.fmm` (10 lines) instead of `foo.ts` (500 lines) for navigation.

Source files are never modified.

**Evidence:**
- Exp13: Inline comments are invisible to LLMs. They treat `// --- FMM ---` blocks as
  code decoration and skip them organically. (research/exp13/FINDINGS.md)
- Exp14: `.fmm/index.json` manifest alone has 0/12 organic discovery rate — LLMs don't
  explore hidden directories without explicit instructions. (research/exp14/FINDINGS.md)
- Exp13: When the LLM does use metadata, token reduction is 88-97% on real codebases
  (research/exp13/BENCHMARKS.md)

**Why not alternatives:**

| Approach | Problem | Evidence |
|----------|---------|----------|
| Inline comments | LLMs skip them | Exp13: 0% organic usage |
| `.fmm/index.json` manifest | LLMs never discover it | Exp14: 0/12 discovery |
| Consolidated map (CODEBASE.md) | Same discovery problem + 152KB blob exceeds source | Exp17 analysis |

### 2. Instruction delivery: Skill + MCP

A skill file (`.claude/skills/fmm-navigate.md`) teaches the LLM WHEN and WHY to use
sidecars. An MCP server provides structured query tools (export lookup, dependency graph,
search). Neither alone is sufficient.

**Evidence (Exp15, 48 runs across 4 conditions x 4 tasks x 3 runs):**

| Condition | Tool Calls | Cost | MCP Adoption |
|-----------|-----------|------|--------------|
| CLAUDE.md only | 22.2 | $0.55 | 83% manifest access |
| Skill only | 22.5 | $0.47 | 0% tool adoption |
| MCP only | 18.2 | $0.50 | 42% tool adoption |
| **Skill + MCP** | **15.5** | **$0.41** | **100% tool adoption** |

Skill + MCP is 30% fewer tool calls, 25% cheaper, 20% faster than CLAUDE.md alone.
The combination is multiplicative: skill provides behavioral guidance, MCP provides
execution capability. (research/exp15/FINDINGS.md, research/exp15-isolated/RESULTS.md)

### 3. Implementation task savings are bounded

For implementation tasks (writing/editing code), the LLM MUST read source files before
editing them. Sidecars reduce exploration reads (finding which files to edit), not
pre-edit reads.

**Realistic targets:** -20% to -40% tool calls, -30% to -50% cost on implementation tasks.

**Evidence:**
- Exp14-quick: -19% tool calls, -50% cost on implementation tasks
  (research/exp14/FINDINGS.md)
- Exp17/17b: LLM reads sidecars AND source files (additive, not substitutive).
  FMM condition: 21 Read calls (6 sidecars + 7 source + 8 other) vs Clean: 12 Read calls.
- Navigation/lookup tasks show much larger gains (88-97% token reduction) because
  the LLM doesn't need to edit anything.

## Anti-patterns

These approaches have been experimentally disproven. Do not rebuild them.

1. **Consolidated map/index files** (CODEBASE.md, index.json as primary navigation).
   A 152KB blob is more context than reading source files directly. Same discovery
   problem as manifest.

2. **Inline comment headers** in source files. LLMs treat comments as noise and skip
   them. Exp13 proved this conclusively.

3. **Manifest-only** (`.fmm/index.json` without instruction delivery). 0/12 organic
   discovery across all conditions. LLMs don't explore hidden directories.

4. **Preamble-only** (text hints without tool integration). Exp17 used
   `--setting-sources ""` which disabled skills and MCP, leaving only a preamble.
   This is the least effective delivery mechanism per Exp15.

5. **Expecting -80% tool call reduction on implementation tasks**. Physically impossible —
   the LLM must read files it edits. Target -20% to -40% for implementation, -80%+ for
   navigation/lookup only.

## Open Questions

1. **How to make sidecars substitutive (not additive) for implementation tasks.**
   Currently the LLM reads sidecars AND source files instead of sidecars INSTEAD OF
   source files for exploration. Exp17/17b showed this clearly.
   (Tracked: ALP-407)

2. **Whether Skill + MCP changes the sidecar equation on large repos.**
   Exp17/17b tested sidecars with preamble-only delivery (the weakest mechanism).
   Exp18 will test sidecars with Skill + MCP (the proven-best mechanism) on Kysely
   (471 files). (Tracked: ALP-405)

3. **What percentage of reads are exploration vs pre-edit.** This determines the
   theoretical maximum improvement from sidecars on implementation tasks.
   (Tracked: ALP-407)

## Experiment Index

| Experiment | Question | Result | Location |
|-----------|----------|--------|----------|
| Exp13 | Do LLMs use inline frontmatter? | No — 0% organic usage, but 88-97% token reduction when forced | research/exp13/ |
| Exp14 | Do LLMs discover manifest organically? | No — 0/12 runs, but CLAUDE.md instructions work immediately | research/exp14/ |
| Exp14-quick | Implementation task savings? | -19% tool calls, -50% cost | research/exp14/ |
| Exp15 | Best instruction delivery mechanism? | Skill + MCP: 30% fewer calls, 25% cheaper, 100% adoption | research/exp15/ |
| Exp15-isolated | Does Docker isolation change results? | No — Skill + MCP still 100% adoption, Skill-only still 0% | research/exp15-isolated/ |
| Exp16 | MCP cost impact (Docker A/B)? | -33% tool calls but only -10% tokens (system prompt overhead) | research/exp16-cost/ |
| Exp17/17b | Sidecars on large repo (Kysely, 471 files)? | -2% to -8% tool calls — sidecars additive, not substitutive | research/exp17-cli/ |
