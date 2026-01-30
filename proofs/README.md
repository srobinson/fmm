# Navigation Proof Harness

Reproducible evidence that `.fmm` sidecar metadata replaces source file reads for LLM codebase navigation.

## What This Proves

An LLM navigating a codebase **with** fmm sidecars uses dramatically fewer file reads, tool calls, and tokens than one navigating **without** — while arriving at the same answer.

| Metric | Without fmm | With fmm | Improvement |
|--------|------------|----------|-------------|
| File reads | ~11 | ~0 | -100% |
| Tool calls | ~22 | ~15 | -30% |
| Tokens | ~7,000 | ~200 | -97% |
| Cost/query | $0.55 | $0.41 | -25% |

*Numbers from experiments 13-15 on real codebases (123-1,306 files). See `research/` for raw data.*

## Background

Five experiments (`research/exp13` through `research/exp17`) validated that:

1. **LLMs skip inline comments** — frontmatter in source comments is invisible (exp13 PIVOT)
2. **Manifest JSON works** — `.fmm/index.json` gives O(1) export/dependency lookups (exp13)
3. **LLMs don't discover manifests organically** — they need a hint via CLAUDE.md or Skill (exp14)
4. **Skill + MCP is optimal** — 30% fewer tool calls, 25% cheaper than instructions alone (exp15)
5. **Token reduction scales** — 88-97% reduction depending on task type (exp13 benchmarks)

This `proofs/` directory distills those findings into a **single reproducible demonstration**.

## Structure

```
proofs/
├── README.md              # This file
├── harness/               # Test harness scripts
│   ├── run-navigation.sh  # Runs control vs treatment navigation queries
│   ├── parse-results.py   # Extracts metrics from stream-json output
│   └── compare-results.py # Generates comparison summary from parsed results
├── content/               # Raw transcripts from proof runs
│   ├── control/           # LLM output without fmm
│   └── treatment/         # LLM output with fmm
├── stats/                 # Computed metrics and comparison tables
│   └── summary.md         # Before/after comparison
└── snippets/              # README-ready copy-paste artifacts
    └── README.md          # Index of available snippets
```

## Running the Proof

```bash
# Prerequisites: Claude CLI (`claude`), python3
cd proofs/
./harness/run-navigation.sh
```

The harness runs a defined navigation query against:
- **Control:** TypeScript auth app with no fmm metadata (`research/exp14/repos/clean/`)
- **Treatment:** same app with `.fmm` manifest + system prompt hint (`research/exp14/repos/hint/`)

It captures tool calls, file reads, token counts, and elapsed time for both conditions, then outputs a comparison table.

## Target Codebase

An 18-file TypeScript auth app (`research/exp14/repos/`) with routes, controllers, middleware, and auth modules — small enough to verify results by hand, large enough to demonstrate navigation patterns.

## Related

- `research/exp13/` — Token reduction benchmarks (88-97%)
- `research/exp14/` — Manifest discovery behavior
- `research/exp15/` — Skill vs MCP comparison (48 runs)
- `experiments/fmm-benchmarking/` — Zustand test protocol
