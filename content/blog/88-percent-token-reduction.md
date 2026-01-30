# We Measured 88-97% Token Reduction for LLM Code Navigation. Here's the Data.

**tl;dr** -- fmm generates a structural manifest of your codebase (exports, imports, dependencies, LOC per file). LLMs read the manifest instead of source files. On a 244-file TypeScript codebase, this reduced lines read by 88-97% for navigation tasks. On a tiny 4-file codebase, it reduced nothing. Both results matter. This post shows the methodology, raw numbers, limitations, and how to reproduce everything.

---

## The Claim

fmm reduces LLM token consumption by 88-97% for codebase navigation tasks.

That range comes from four task types tested against a real codebase (agentic-flow: 244 TypeScript files, 81,732 lines of code). The low end (88%) is refactor impact analysis, where the LLM still needs to read some source. The high end (97.5%) is architecture exploration, where structural metadata is almost entirely sufficient.

We also ran a test where fmm provided approximately 0% improvement. We're reporting that too.

---

## What fmm Produces

fmm is a Rust CLI that uses tree-sitter to parse source files and generate structured metadata. For each file in your codebase, it produces a sidecar entry containing:

```yaml
# .fmm/index.json (excerpt for one file)
{
  "file": "src/proxy/adaptive-proxy.ts",
  "exports": ["AdaptiveProxy", "createProxy", "ProxyConfig"],
  "imports": ["crypto", "fs", "http2"],
  "dependencies": [
    "../utils/logger.js",
    "./anthropic-to-gemini.js",
    "./http2-proxy.js",
    "./http3-proxy.js",
    "./websocket-proxy.js"
  ],
  "loc": 487
}
```

One manifest file. Every file's public API, dependency graph, and size. An LLM reads this once and knows where everything lives without opening a single source file.

---

## Methodology

### Test Codebase

**agentic-flow** -- a production TypeScript monorepo:
- 244 source files (exp13) / 1,306 total files (exp15)
- 81,732 lines of code
- 3,426 exports across all files
- Realistic structure: API routes, middleware, services, utilities, tests

### Isolation

Experiments used Claude Code's isolation flags to prevent context leakage:

```bash
claude --setting-sources ""          # No user/project settings
       --strict-mcp-config '{}'      # No ambient MCP servers
       --no-session-persistence      # No state between runs
       --system-prompt "..."         # Clean, controlled prompt
```

No shared filesystem state. No cached sessions. No global CLAUDE.md bleeding across conditions.

For the delivery mechanism experiment (exp15), we ran 48 total runs: 4 conditions x 4 task types x 3 runs per condition. Docker-based full isolation was also prepared (separate containers per run, network disabled, tmpfs for temp directories) to validate that ambient configuration wasn't compressing observed deltas.

### Task Types

Four categories designed to cover the spectrum of how LLMs interact with codebases:

1. **Code review** -- "Review what changed this week and summarize"
2. **Refactor analysis** -- "Analyze impact of adding a parameter to loadConfig()"
3. **Security review** -- "Find security bugs in this codebase" (on a 4-file, 123-LOC test project)
4. **Architecture exploration** -- "Understand how the swarm system works" (on the full 244-file codebase)

### Control vs Treatment

| | Control | Treatment |
|---|---|---|
| Codebase | Raw source files | Same source files + `.fmm/index.json` |
| Instructions | None | CLAUDE.md entry: "Check .fmm/ for codebase index" |
| Model | Claude Opus 4.5 | Claude Opus 4.5 |
| Isolation | Full | Full |

The treatment adds exactly two things: the manifest file and a one-line instruction telling the LLM it exists.

---

## Results

### Per-Task Breakdown

| Task | Control (lines read) | fmm (lines read) | Reduction | Notes |
|------|--------------------:|------------------:|----------:|-------|
| Code review | 1,824 | 65 | **96.4%** | Manifest provided change context without full file reads |
| Refactor analysis | 2,800 | 345 | **87.7%** | More tool calls but far fewer lines per call |
| Security review (4 files, 123 LOC) | 123 | 120 | **~0%** | Must read every line for security audit |
| Architecture exploration | 7,135 | 180 | **97.5%** | Zero full file reads; frontmatter alone was sufficient |

### What the LLM Actually Did

**Control behavior (no fmm):**
```
grep "thing" -> find files -> read entire files -> understand code
```

The LLM's default strategy is brute force. It greps for keywords, opens every matching file, reads it top to bottom, then synthesizes. On Test 3 (architecture exploration), this meant reading 12 files at an average of 595 lines each.

**Treatment behavior (with fmm):**
```
read .fmm/index.json -> identify relevant files -> read first 15 lines (metadata) -> done
```

With the manifest, the LLM's first action was always `Read(.fmm/index.json)`. It got a structural map of the entire codebase in one read, then selectively opened only the files it needed -- and often only read the metadata header, not the full source.

On Test 3, the fmm agent analyzed the same 12 files but read only 180 total lines (12 x 15 lines of metadata). Its own summary: "No full file reads were necessary -- frontmatter provided complete dependency and export information."

Both agents produced equivalent architecture diagrams and export documentation. Same output quality, 97.5% less input.

### The Refactor Anomaly

Test 1 (refactor analysis) is interesting: the fmm agent made **more** tool calls (23 vs 17) but read **far fewer** lines (345 vs 2,800). It also identified more affected files (20+ vs 14).

The manifest gave it a dependency graph. Instead of deep-reading a few files, it did many quick lookups across the manifest to trace the full impact chain. More calls, less data per call, better coverage. The strategy shifted from "read everything in a few files" to "peek at metadata across many files."

---

## When fmm Does Not Help

### Security Review on Small Codebases

Test 2 was deliberately designed to find the floor. A 4-file codebase with 123 lines of code and 6 planted security bugs.

Result: 123 lines read (control) vs 120 lines read (fmm). Effectively zero improvement.

This is correct behavior. Security review requires reading actual code, not metadata. You cannot find an SQL injection vulnerability by looking at an exports list. And on a 4-file codebase, the manifest overhead (reading the index, parsing metadata) is proportionally large compared to just reading the files directly.

### The Crossover Point

fmm has a startup cost: reading and parsing the manifest. It pays off when triage savings exceed that cost.

```
If avg file = 100 lines
And metadata = ~10 lines per entry

Skip 1 in 10 files -> break even
Skip more -> fmm wins
Skip fewer -> fmm loses
```

In practice, real codebases are large. The break-even case (tiny codebase, line-by-line audit task) is not representative of how LLMs typically interact with production code.

### Tasks That Always Require Full Reads

- Line-by-line security audits
- Code style/formatting reviews
- Logic verification for specific functions
- Test coverage analysis of implementation details

fmm helps with navigation (finding where things are and how they connect). It does not help with comprehension tasks that require reading actual implementation.

---

## Delivery Mechanism Matters

We didn't stop at proving the manifest works. We tested *how* to deliver it to the LLM (exp15: 48 runs, 4 conditions, 4 task types, 3 runs each).

### Conditions

| Condition | Description |
|---|---|
| A: CLAUDE.md only | One-line instruction in project config |
| B: Skill only | Installable instruction package (Claude Code skill) |
| C: MCP only | fmm MCP server registered, no instructions |
| D: Skill + MCP | Both behavioral guidance and structured query tools |

### Results

| Condition | Avg Tool Calls | Avg Cost | Manifest Access Rate |
|---|---:|---:|---:|
| A: CLAUDE.md only | 22.2 | $0.55 | 83% |
| B: Skill only | 22.5 | $0.47 | 75% |
| C: MCP only | 18.2 | $0.50 | 58% |
| **D: Skill + MCP** | **15.5** | **$0.41** | **75%** |

Skill + MCP is strictly best: **30% fewer tool calls** than CLAUDE.md alone, **25% cheaper**, **20% faster** (68.5s vs 85.8s average).

The skill provides behavioral guidance ("check the manifest first"). The MCP server provides structured queries (`fmm_lookup_export`, `fmm_dependency_graph`). Neither alone is optimal.

Notable: MCP alone (condition C) still achieved 58% manifest access without any instructions. Tool descriptions alone triggered some manifest-aware behavior. But it missed the manifest in 42% of runs, particularly for dependency mapping tasks where the LLM never thought to check.

---

## Economics

### Per-Query Cost Modeling

Using actual API pricing (as of January 2026):

| Model | Price (input) | 100-file scan without fmm | 100-file scan with fmm | Savings |
|---|---:|---:|---:|---:|
| Claude Opus 4.5 | $15.00/1M tokens | ~$0.75 | ~$0.024 | 97% |
| Claude Sonnet 4.5 | $3.00/1M tokens | ~$0.15 | ~$0.005 | 97% |
| GPT-4o | $2.50/1M tokens | ~$0.13 | ~$0.004 | 97% |

Assumptions: average file is 200 lines (~500 tokens), manifest entry is ~10 lines (~25 tokens). A "100-file scan" means the LLM reads all 100 files (control) vs reads the manifest + 5 targeted files (treatment).

### Annual Projections

These compound fast when LLM-assisted development is a daily activity:

| Usage Profile | Queries/Day | Annual Without fmm | Annual With fmm | Saved |
|---|---:|---:|---:|---:|
| Solo developer | 20-40 | $6K-12K | $200-400 | **$6K-12K** |
| Small team (5 devs) | 100-200 | $30K-60K | $1K-2K | **$29K-58K** |
| Enterprise (50 devs) | 1,000-2,000 | $300K-600K | $10K-20K | **$290K-580K** |

These are rough projections based on Claude Opus 4.5 pricing and assume navigation-heavy workflows. Your actual numbers depend on model choice, query complexity, and codebase size. The percentage reduction (88-97%) is empirically validated; the dollar figures are extrapolations.

---

## Case Study: claude-flow Issue #1044

Real-world validation on a codebase we don't own.

**Repo:** [ruvnet/claude-flow](https://github.com/ruvnet/claude-flow) -- 9,008 files
**Bug:** Model selected in Claude Code is not used (#1044)

```bash
$ git clone ruvnet/claude-flow    # 9,008 files
$ fmm init                        # 3 seconds
# Generated .fmm/index.json (2,221 source files indexed)
```

### Without fmm (estimated)

Grep 9,000 files for "model", "opus", "sonnet". Hundreds of matches across docs, configs, tests, node_modules. Open file after file tracing the model selection flow. Estimated: 30-50 files read, 40-60 tool calls.

### With fmm

The manifest mapped the codebase structure. Navigated straight to 2 files:

```
v3/@claude-flow/cli/src/services/headless-worker-executor.ts
v3/@claude-flow/cli/src/runtime/headless.ts
```

Three root causes found. Five-line fix. Two files changed. PR merged: [#1050](https://github.com/ruvnet/claude-flow/pull/1050).

| Metric | Without fmm (est.) | With fmm |
|---|---|---|
| Files in repo | 9,008 | 9,008 |
| Files indexed | -- | 2,221 |
| Files read to find bug | ~30-50 | 2 |
| Tool calls | ~40-60 | ~10 |
| Fix size | same | 5 lines, 2 files |

fmm turned a haystack problem into a lookup problem.

---

## Comparison with Alternatives

| Approach | What it provides | Infrastructure | Deterministic | Cross-file index | Dependency graph |
|---|---|---|---|---|---|
| **RAG / embeddings** | Semantic search over code | Vector DB, embedding model, indexing pipeline | No | Partial | No |
| **Tree-sitter (raw)** | AST per file | None (library) | Yes | No | No |
| **ctags / LSP** | Symbol index | Tag file or language server | Yes | Symbol-level only | No |
| **fmm** | Exports, imports, deps, LOC per file | None (single binary) | Yes | Yes | Yes |

**RAG/embeddings** are powerful for semantic queries ("find code that handles authentication") but require infrastructure: a vector database, an embedding model, an indexing pipeline. Results are probabilistic. fmm is deterministic and answers structural queries ("what does this file export, what depends on it") with zero infrastructure.

**Raw tree-sitter** gives you an AST for individual files but no cross-file relationships. You can parse one file's exports but you can't ask "what depends on this file?" without building your own index. That index is what fmm builds.

**ctags** provides symbol-level indexing (function definitions, class names) but not imports, dependencies, or line counts. It tells you where `createProxy` is defined but not what files import it or what it depends on.

fmm occupies a specific niche: **deterministic, zero-infrastructure, file-level structural metadata with cross-file dependency tracking.** It doesn't replace semantic search. It replaces the brute-force grep-and-read pattern that LLMs default to for structural navigation.

---

## Reproducibility

All experiment data, scripts, and raw traces are in the repository:

```
research/
  exp13/          # Core 88-97% reduction experiments
    FINDINGS.md   # Full results and analysis
  exp14/          # LLM manifest discovery experiments (12 runs)
    FINDINGS.md   # Finding: LLMs never discover .fmm/ organically
    results/      # Raw traces per run
  exp15/          # Delivery mechanism comparison (48 runs)
    FINDINGS.md   # CLAUDE.md vs Skill vs MCP vs Skill+MCP
    results/      # Raw traces per condition/task/run
    run-exp15.sh  # Execution script
  exp15-isolated/ # Docker-based full isolation variant
    setup.sh      # Environment setup
    run-isolated.sh
  exp16-cost/     # Structured cost benchmarks
    tasks.json    # 10 task definitions with ground truth
    results/      # Scored results (condition A vs B)
    score.py      # Automated scoring script
```

### To Reproduce exp13 (Core Results)

```bash
git clone https://github.com/srobinson/fmm
cd fmm/research/exp13

# The test codebase (agentic-flow) is referenced in the findings.
# Control: run Claude Code against raw codebase with isolation flags
# Treatment: run against same codebase with .fmm/index.json present

# Isolation flags used:
claude --setting-sources "" \
       --strict-mcp-config '{}' \
       --no-session-persistence \
       --system-prompt "Your task is: ..."
```

### To Reproduce exp15 (Delivery Mechanisms)

```bash
cd research/exp15
./run-exp15.sh          # All 48 runs
python3 parse-results.py # Aggregate results
```

### To Reproduce exp16 (Structured Cost Benchmarks)

```bash
cd research/exp16-cost
./run.sh                # Run all 10 tasks, both conditions
python3 score.py        # Score against ground truth
```

---

## Limitations

We want to be explicit about what this research does and does not show.

**What we validated:**
- 88-97% reduction in lines read for navigation tasks on codebases with 100+ files
- The manifest approach works when the LLM is instructed to use it
- Delivery via Skill + MCP is the most efficient mechanism
- The approach works on real codebases we don't own (claude-flow, 9,008 files)

**What we did not validate:**
- Cross-model generalization (tested primarily on Claude Opus 4.5 and Sonnet 4.5; not tested on GPT-4o, Gemini, or open-source models)
- Languages beyond TypeScript (tree-sitter supports many languages; the experiments used TypeScript exclusively)
- Long-session effects (all tests were single-query; we haven't measured manifest staleness in multi-hour sessions)
- Team-scale deployment (the claude-flow case study is a single-developer workflow)

**Honest caveats:**
- The 0% reduction on the 4-file security review is a real limitation, not an edge case. Any task requiring full source reads will not benefit.
- "88-97%" is the range across four specific task types on one codebase. Your mileage will vary based on codebase size, task type, and model.
- The annual cost projections are extrapolations, not measurements. The per-query reduction percentages are measured; the dollar figures are modeled.
- LLMs do not discover the manifest organically. Across 12 isolated runs in exp14, 0/12 found `.fmm/index.json` without being told about it. The instruction is required.

---

## Conclusion

LLMs navigate codebases by reading files. On real codebases, they read far more than they need to. A structural manifest -- exports, imports, dependencies, LOC per file -- gives the LLM a map. With that map, it reads 88-97% fewer lines for navigation tasks while producing equivalent output.

The mechanism is simple: instead of grepping and reading every matching file, the LLM reads one manifest and makes targeted reads. The economics compound with codebase size and query frequency.

fmm generates that manifest. One command, sub-second on codebases with thousands of files, zero infrastructure, deterministic output.

The data is in the repo. Run the experiments yourself.

---

*Research conducted January 2026. All experiments used Claude Opus 4.5 and Claude Sonnet 4.5. Raw traces and reproduction scripts available in the `research/` directory. fmm is open source.*
