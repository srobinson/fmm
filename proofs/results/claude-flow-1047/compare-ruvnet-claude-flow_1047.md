## fmm gh issue --compare Results

**Issue:** ruvnet/claude-flow#1047 — Statusline ADR count is hardcoded to 0/0
**Model:** sonnet | **Budget:** $5.00 | **Max turns:** 30
**Timestamp:** 2026-01-31T17:26:03.887779+00:00

| Metric | Control | FMM | Delta | Savings |
|--------|---------|-----|-------|---------|
| Input tokens | 26 | 18 | -8 | 31% |
| Output tokens | 7.8K | 10.3K | +2434 | — |
| Cache read tokens | 1.7M | 2.4M | +647.7K | — |
| Total cost | $0.91 | $1.22 | +$0.31 | -34% |
| Turns | 31 | 29 | -2 | 6% |
| Tool calls | 35 | 28 | -7 | 20% |
| Files read | 9 | 10 | +1 | -11% |
| Duration | 210s | 246s | +36s | -17% |

**Verdict:** fmm did not reduce token usage in this run.
