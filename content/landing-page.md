# Stop Paying Your AI to Read Files It Doesn't Need

## 97% fewer tokens per navigation query. Same answers.

fmm generates lightweight metadata sidecars for every source file, so your AI navigates by structure instead of brute-forcing through raw code.

```bash
cargo install fmm && fmm init && fmm generate
```

---

## The Problem

Every time your AI reads a file, you pay for tokens it doesn't need.

Your AI assistant doesn't know where `createStore` is defined. So it reads files until it finds it. You pay for every token of every file it opens.

| | Without fmm | With fmm |
|---|---|---|
| **Files touched** | 100 source files | 100 `.fmm` sidecars |
| **Tokens consumed** | ~50,000 | ~1,500 |
| **Cost at $3/MTok** | $0.15 per query | $0.0045 per query |
| **Reduction** | -- | **97%** |

Multiply that by hundreds of queries per day across a team. The waste compounds fast.

---

## How It Works

### 1. Initialize

```bash
fmm init
```

Creates your `.fmm.yaml` config and sets up LLM integration (CLAUDE.md rules, MCP server, or both).

### 2. Generate sidecars

```bash
fmm generate
```

Every source file gets a `.fmm` companion with structured metadata. Regenerate anytime your code changes.

### 3. Your AI navigates metadata, not source

Instead of opening `src/store/index.ts` (487 lines, 2,100 tokens), your AI reads `src/store/index.ts.fmm`:

```yaml
---
file: src/store/index.ts
loc: 487
imports:
  - { from: "./types", symbols: [StoreConfig, State] }
  - { from: "../events/emitter", symbols: [EventEmitter] }
exports:
  - { name: createStore, type: function, line: 45 }
  - { name: Store, type: class, line: 89 }
  - { name: StoreOptions, type: interface, line: 12 }
dependencies:
  - ../events/emitter
  - ./types
  - ./middleware
---
```

**32 tokens instead of 2,100.** Your AI knows what the file exports, what it imports, and where to look next -- without reading a single line of source code.

---

## Evidence

These are measured results, not estimates.

### 88-97% token reduction (Exp13)

A 244-file TypeScript codebase. Navigation queries that consumed ~50K tokens dropped to ~1.5K tokens. Exposed file contents replaced with structured metadata.

### Proven across 48+ experimental runs (Exp15)

Systematic A/B testing across multiple AI configurations (Skill-only, MCP-only, Skill+MCP). Consistent reductions in tool calls and source file reads across all configurations.

### Case study: claude-flow (Exp14)

9,008 files. Full sidecar index generated in 3 seconds. AI located a cross-module bug in 2 file reads that previously required scanning dozens of files.

### Proof harness: live A/B comparison (Exp15)

Head-to-head on architecture navigation queries:

- **36% fewer tool calls** (fmm vs. baseline)
- **53% fewer source file reads** (fmm vs. baseline)
- **30% fewer tool calls** with Skill+MCP configuration

---

## Why fmm

### Save tokens, save money

97% fewer tokens per navigation query (Exp13). For a small team running hundreds of AI queries daily, that translates to **$6K-25K/year in reduced API costs**. The savings scale linearly with team size and query volume.

### Faster AI responses

Fewer tool calls means faster answers. Your AI spends less time reading irrelevant code and more time solving your problem. 30% fewer tool calls measured with Skill+MCP integration (Exp15). Fewer round-trips to the API, shorter wait times.

### Works with your existing tools

Claude Code, Cursor, Windsurf, any MCP-compatible client. Three commands to set up. Zero changes to how you prompt or interact with your AI. fmm integrates through CLAUDE.md instructions, MCP server, or both -- your AI starts using sidecars automatically.

---

## Built for How You Work

### For developers

Better context for your AI pair programmer. Your AI understands codebase structure before it reads a single source file. Fewer wrong turns, fewer wasted reads, better answers on the first try.

### For AI/LLM engineering teams

Reduce API costs by 88-97% per navigation query (Exp13). Structured metadata gives your tooling a reliable, parseable map of any codebase. Build smarter retrieval pipelines on top of fmm's output.

### For enterprises

Infrastructure for scaling LLM-driven development. Consistent metadata across repositories. Predictable costs as AI usage grows. Integrates into CI/CD -- regenerate sidecars on every push.

### For open source maintainers

Help contributors and AI tools navigate your project faster. A `.fmm` sidecar index is a machine-readable map of your codebase. New contributors and their AI assistants orient in seconds, not minutes.

---

## Get Started

```bash
cargo install fmm && fmm init && fmm generate
```

That's it. Three commands. Your AI starts navigating smarter immediately.

**GitHub:** [github.com/srobinson/fmm](https://github.com/srobinson/fmm)

**Deep dives:**
- [Experiment 13: Token reduction measurements](https://github.com/srobinson/fmm/blob/main/docs/experiments/exp13.md)
- [Experiment 14: claude-flow case study (9,008 files)](https://github.com/srobinson/fmm/blob/main/docs/experiments/exp14.md)
- [Experiment 15: A/B proof harness results](https://github.com/srobinson/fmm/blob/main/docs/experiments/exp15.md)
