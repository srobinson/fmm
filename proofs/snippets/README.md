# Proof Snippets

Copy-paste-ready artifacts from the fmm navigation proof. Use in README, landing page, or pitch deck.

All numbers from live proof runs (2026-01-30) on an 18-file TypeScript auth app.

## Available Snippets

| File | What it shows |
|------|---------------|
| [`headline.md`](headline.md) | The key stat: 36% fewer tool calls, 53% fewer reads, 31% fewer tokens |
| [`side-by-side.md`](side-by-side.md) | Full before/after comparison with navigation paths |
| [`tool-call-trace.md`](tool-call-trace.md) | Every tool call the LLM made, control vs treatment |
| [`manifest-excerpt.md`](manifest-excerpt.md) | What `.fmm/index.json` looks like (the manifest the LLM reads) |
| [`query.md`](query.md) | The exact navigation question asked |

## Quick Copy

The most impactful single stat for a README:

> **36% fewer tool calls. 53% fewer source reads. 31% fewer tokens.** Same architectural understanding. The LLM reads the manifest first, then only opens files it needs.
