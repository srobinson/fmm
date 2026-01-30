## Headline Stats

### On 18-file codebase (this proof)

**36% fewer tool calls. 53% fewer source reads. 31% fewer tokens.**

The LLM read the manifest first, then only opened files it needed. Without fmm, it read every file.

### On larger codebases (prior experiments)

| Codebase | Files | Token reduction | Source |
|----------|-------|----------------|--------|
| mdcontext | 123 | **88-97%** | research/exp13 |
| agentic-flow | 1,306 | **30% fewer tool calls, 25% cheaper** | research/exp15 |

fmm's benefit scales with codebase size. On small codebases, the LLM can brute-force read everything. On real-world projects, that strategy doesn't scale — and fmm gives the LLM a map.
