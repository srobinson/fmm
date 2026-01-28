# FMM Benchmarking Methodology

**Purpose:** Demonstrate and quantify fmm's value proposition - token savings and improved LLM code navigation.

**Status:** Methodology design complete. Ready for implementation.

---

## Executive Summary

This document defines a rigorous benchmarking methodology to prove fmm reduces LLM token consumption by 88-97% while maintaining navigation accuracy. The methodology uses controlled experiments across three TypeScript repositories of varying sizes.

---

## Test Repositories

### Selection Criteria

1. **Open source** - Freely accessible, reproducible results
2. **TypeScript** - fmm's primary supported language
3. **Active/Maintained** - Real-world complexity, not toy projects
4. **Well-structured** - Clean imports/exports (tree-sitter can parse)
5. **Varying sizes** - Test crossover points and scalability

### Recommended Repositories

#### Small (~50 files): **zustand**
- **Repo:** https://github.com/pmndrs/zustand
- **Stars:** 56k+
- **Why:** Clean, modern TypeScript. Minimal dependencies. ~30-50 source files.
- **Characteristics:** Single-purpose library, well-organized exports, clear dependency tree
- **Clone:** `git clone --depth 1 https://github.com/pmndrs/zustand.git`

#### Medium (~200 files): **hono**
- **Repo:** https://github.com/honojs/hono
- **Stars:** 25k+
- **Why:** Web framework with middleware, adapters, utilities. ~150-250 source files.
- **Characteristics:** Multi-module structure, various entry points, internal dependencies
- **Clone:** `git clone --depth 1 https://github.com/honojs/hono.git`

#### Large (~1000+ files): **excalidraw**
- **Repo:** https://github.com/excalidraw/excalidraw
- **Stars:** 111k+
- **Why:** Full application with UI, state, utilities. ~800-1200 source files.
- **Characteristics:** Complex dependency graph, many modules, production-grade
- **Clone:** `git clone --depth 1 https://github.com/excalidraw/excalidraw.git`

### Alternative Options

| Size | Primary | Alternative 1 | Alternative 2 |
|------|---------|---------------|---------------|
| Small | zustand | ink | zodios |
| Medium | hono | trpc | payload (cms) |
| Large | excalidraw | vscode-extension-samples | next.js (examples) |

---

## Metrics to Capture

### Primary Metrics

| Metric | Unit | How to Measure | Why It Matters |
|--------|------|----------------|----------------|
| **Tokens Read** | count | Sum of all tokens in files/manifest read | Direct cost savings |
| **Lines Read** | count | Sum of all lines read by LLM | Proxy for tokens (easier to measure) |
| **Files Accessed** | count | Number of unique files opened | Navigation efficiency |
| **Read Tool Calls** | count | Number of Read operations | API call overhead |
| **Task Completion Time** | seconds | Wall clock time to answer | User experience |

### Secondary Metrics

| Metric | Unit | How to Measure | Why It Matters |
|--------|------|----------------|----------------|
| **Accuracy Score** | 0-100% | Correct answers / Total questions | Quality baseline |
| **Context Window Usage** | % | Tokens used / Context limit | Headroom for reasoning |
| **Full File Reads** | count | Files read entirely vs. partially | Triage effectiveness |
| **Manifest Queries** | count | Times manifest was consulted | fmm utilization |

### Token Calculation

```
tokens_read = sum(file_lines * avg_tokens_per_line)

Where avg_tokens_per_line ~= 8 for TypeScript
(empirically: 1 line = 6-10 tokens depending on density)
```

For precise measurement, use tiktoken (OpenAI) or anthropic's token counter.

---

## Test Scenarios

### Scenario 1: Find Export Location

**Task:** "Which file exports `<SYMBOL_NAME>`?"

**Purpose:** Test direct symbol lookup - the most common navigation task.

**Methodology:**
1. Pick 5 exports per repo (varying: common, rare, nested)
2. Ask LLM to find the file without hints
3. Measure reads until correct answer

**Expected Symbols (per repo):**
- zustand: `create`, `useStore`, `persist`, `devtools`, `subscribeWithSelector`
- hono: `Hono`, `Context`, `cors`, `jwt`, `serveStatic`
- excalidraw: `App`, `exportToCanvas`, `useCallbackRefState`, `getSceneVersion`, `actionClearCanvas`

**Success Criteria:** Correct file identified

---

### Scenario 2: Trace Import Chain

**Task:** "What files import `<MODULE_NAME>`?"

**Purpose:** Test reverse dependency lookup - critical for refactoring.

**Methodology:**
1. Pick 3 commonly-imported modules per repo
2. Ask LLM to list all files that import it
3. Compare against ground truth (grep result)

**Expected Modules:**
- zustand: `./vanilla`, `./middleware`, `./context`
- hono: `./context`, `./hono-base`, `./utils/url`
- excalidraw: `./data/restore`, `./scene`, `./constants`

**Success Criteria:** >= 90% recall of actual importers

---

### Scenario 3: Dependency Chain Trace

**Task:** "Trace the dependency chain from `<FILE_A>` to `<FILE_B>`"

**Purpose:** Test deep navigation - understanding how code connects.

**Methodology:**
1. Pick 2 file pairs per repo (2-4 hops apart)
2. Ask LLM to trace the import path
3. Verify path is valid

**Expected Pairs:**
- zustand: `src/vanilla.ts` -> `src/middleware/persist.ts`
- hono: `src/hono.ts` -> `src/middleware/cors/index.ts`
- excalidraw: `packages/excalidraw/index.tsx` -> `packages/excalidraw/data/json.ts`

**Success Criteria:** Valid path found

---

### Scenario 4: Architecture Understanding

**Task:** "Explain the module structure of this codebase"

**Purpose:** Test holistic understanding - overall codebase comprehension.

**Methodology:**
1. Ask LLM to describe the architecture
2. Grade answer against known structure
3. Check for: main modules, dependencies, entry points

**Success Criteria:** Covers >= 80% of major modules

---

### Scenario 5: Refactor Impact Analysis

**Task:** "If I change the signature of `<FUNCTION>`, what files need updating?"

**Purpose:** Test practical development task - the highest-value use case.

**Methodology:**
1. Pick 1 function per repo with moderate usage (5-15 call sites)
2. Ask LLM to identify impact
3. Compare against grep/ast analysis

**Expected Functions:**
- zustand: `createStore` signature change
- hono: `Context.json()` return type change
- excalidraw: `restoreElements` parameter change

**Success Criteria:** >= 85% of affected files identified

---

## Comparison Methodology

### Three Test Conditions

```
┌─────────────────────────────────────────────────────────────────┐
│                          CONDITIONS                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. BASELINE (Control)                                          │
│     LLM reads files directly using grep + read                  │
│     No manifest, no frontmatter, no hints                       │
│                                                                  │
│  2. FMM MANIFEST                                                │
│     LLM has access to .fmm/manifest.json                        │
│     Instructed: "Query manifest before reading files"           │
│                                                                  │
│  3. FMM MCP (Optional)                                          │
│     LLM uses MCP tools: fmm_find_export, fmm_search, etc.       │
│     Tools abstract manifest queries                             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Ensuring Fair Comparison

| Factor | How to Control |
|--------|----------------|
| **Same Model** | Use identical model for all conditions (Claude Opus 4.5) |
| **Same Prompts** | Identical task descriptions, only system context differs |
| **Fresh Context** | New conversation per test (no memory carry-over) |
| **Randomized Order** | Vary condition order to avoid learning effects |
| **Multiple Runs** | 3 runs per condition, report mean + variance |
| **Blind Grading** | Grade outputs without knowing which condition |

### System Prompts per Condition

**Baseline (Control):**
```
You are a code navigation assistant. Answer questions about the codebase.
You have access to: grep (search), read (view files), glob (find files).
The codebase is located at: /path/to/repo
```

**FMM Manifest:**
```
You are a code navigation assistant. Answer questions about the codebase.
You have access to: grep (search), read (view files), glob (find files).

IMPORTANT: A manifest file exists at .fmm/manifest.json containing:
- All files with their exports, imports, and dependencies
- File metadata (lines of code, last modified)
Query this manifest FIRST before reading individual files.

The codebase is located at: /path/to/repo
```

**FMM MCP:**
```
You are a code navigation assistant. Answer questions about the codebase.
You have access to fmm tools:
- fmm_find_export(name) - Find file by export name
- fmm_list_exports(file) - List exports from a file
- fmm_search(query) - Search manifest with filters
- fmm_get_manifest() - Get full project structure
- fmm_file_info(file) - Get file metadata

Use these tools for navigation. Only read full files when necessary.
```

---

## Benchmark Script Outline

### Directory Structure

```
benchmark/
├── scripts/
│   ├── setup.sh              # Clone repos, generate manifests
│   ├── run_benchmark.ts      # Main benchmark runner
│   ├── measure_tokens.ts     # Token counting utilities
│   ├── grade_responses.ts    # Accuracy grading
│   └── aggregate_results.ts  # Generate summary reports
├── tasks/
│   ├── zustand.json          # Task definitions for zustand
│   ├── hono.json             # Task definitions for hono
│   └── excalidraw.json       # Task definitions for excalidraw
├── results/
│   ├── raw/                  # Raw LLM outputs
│   └── summary/              # Aggregated metrics
└── repos/                    # Cloned test repositories
```

### setup.sh

```bash
#!/bin/bash
set -e

# Clone repos
mkdir -p benchmark/repos
cd benchmark/repos

git clone --depth 1 https://github.com/pmndrs/zustand.git
git clone --depth 1 https://github.com/honojs/hono.git
git clone --depth 1 https://github.com/excalidraw/excalidraw.git

# Generate fmm manifests
cd ../..
for repo in zustand hono excalidraw; do
  fmm generate --manifest-only benchmark/repos/$repo/src
done

# Count files for baseline
for repo in zustand hono excalidraw; do
  echo "$repo: $(find benchmark/repos/$repo/src -name '*.ts' -o -name '*.tsx' | wc -l) TypeScript files"
done
```

### run_benchmark.ts (Pseudocode)

```typescript
interface Task {
  id: string;
  scenario: 'find_export' | 'trace_import' | 'dependency_chain' | 'architecture' | 'refactor_impact';
  prompt: string;
  expected: string[];  // Ground truth answers
  repo: string;
}

interface BenchmarkResult {
  taskId: string;
  condition: 'baseline' | 'manifest' | 'mcp';
  run: number;
  metrics: {
    linesRead: number;
    filesAccessed: number;
    readCalls: number;
    tokensUsed: number;
    timeMs: number;
    accuracy: number;
  };
  rawOutput: string;
}

async function runBenchmark(
  task: Task,
  condition: 'baseline' | 'manifest' | 'mcp',
  run: number
): Promise<BenchmarkResult> {

  // 1. Initialize clean conversation
  const conversation = new Conversation({
    model: 'claude-opus-4-5-20251101',
    systemPrompt: getSystemPrompt(condition, task.repo),
    tools: getTools(condition),
  });

  // 2. Instrument tools to capture metrics
  const metrics = {
    linesRead: 0,
    filesAccessed: new Set<string>(),
    readCalls: 0,
    tokensUsed: 0,
  };

  conversation.on('tool_call', (call) => {
    if (call.name === 'read') {
      metrics.readCalls++;
      metrics.filesAccessed.add(call.args.file);
      metrics.linesRead += countLines(call.result);
    }
  });

  // 3. Run task
  const startTime = Date.now();
  const response = await conversation.send(task.prompt);
  const timeMs = Date.now() - startTime;

  // 4. Calculate tokens
  metrics.tokensUsed = countTokens(response.inputTokens + response.outputTokens);

  // 5. Grade accuracy
  const accuracy = gradeResponse(response.text, task.expected);

  return {
    taskId: task.id,
    condition,
    run,
    metrics: {
      ...metrics,
      filesAccessed: metrics.filesAccessed.size,
      timeMs,
      accuracy,
    },
    rawOutput: response.text,
  };
}

// Main execution
async function main() {
  const tasks = loadTasks();
  const results: BenchmarkResult[] = [];

  for (const task of tasks) {
    for (const condition of ['baseline', 'manifest', 'mcp'] as const) {
      for (let run = 1; run <= 3; run++) {
        console.log(`Running: ${task.id} / ${condition} / run ${run}`);
        const result = await runBenchmark(task, condition, run);
        results.push(result);
        await sleep(2000); // Rate limiting
      }
    }
  }

  saveResults(results);
  generateReport(results);
}
```

### Task Definition Format (tasks/zustand.json)

```json
{
  "repo": "zustand",
  "repo_path": "benchmark/repos/zustand",
  "tasks": [
    {
      "id": "zustand-find-create",
      "scenario": "find_export",
      "prompt": "Which file exports the 'create' function?",
      "expected": ["src/vanilla.ts", "src/react.ts"],
      "ground_truth_method": "grep -r 'export.*create' src/"
    },
    {
      "id": "zustand-imports-vanilla",
      "scenario": "trace_import",
      "prompt": "What files import from './vanilla'?",
      "expected": ["src/react.ts", "src/index.ts"],
      "ground_truth_method": "grep -r \"from './vanilla'\" src/"
    },
    {
      "id": "zustand-refactor-createStore",
      "scenario": "refactor_impact",
      "prompt": "If I add a required 'name' parameter to createStore(), which files need updating?",
      "expected": ["src/vanilla.ts", "src/react.ts", "tests/*.ts"],
      "ground_truth_method": "grep -r 'createStore(' src/ tests/"
    }
  ]
}
```

---

## Measurement Approach

### Token Measurement

**Option 1: Direct Token Count (Preferred)**
```typescript
import Anthropic from '@anthropic-ai/sdk';

// Use API response metadata
const response = await anthropic.messages.create({...});
const inputTokens = response.usage.input_tokens;
const outputTokens = response.usage.output_tokens;
```

**Option 2: Line-Based Estimate**
```typescript
// When direct count unavailable
const TOKENS_PER_LINE = 8;  // Empirical average for TypeScript
const estimatedTokens = linesRead * TOKENS_PER_LINE;
```

### Accuracy Grading

**Automated Grading (for objective tasks):**
```typescript
function gradeResponse(response: string, expected: string[]): number {
  // For find_export: exact file match
  // For trace_import: recall score (found / total)
  // For refactor_impact: F1 score

  const found = extractFileNames(response);
  const intersection = found.filter(f => expected.includes(f));

  const precision = intersection.length / found.length;
  const recall = intersection.length / expected.length;

  return 2 * (precision * recall) / (precision + recall);  // F1
}
```

**Manual Grading (for subjective tasks):**
```typescript
// For architecture understanding
// Rubric: 0-100 based on coverage of key modules
const rubric = {
  'identifies entry point': 20,
  'lists major modules': 30,
  'describes dependencies': 20,
  'notes patterns used': 15,
  'accurate details': 15,
};
```

### Fair Comparison Protocol

1. **Same Random Seed:** Fix seed for any randomized elements
2. **Temperature = 0:** Deterministic outputs where possible
3. **No Caching:** Disable response caching
4. **Fresh Environment:** Clean tool state per run
5. **Ground Truth First:** Establish correct answers before any runs

---

## Expected Results Format

### Summary Table

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    FMM BENCHMARK RESULTS SUMMARY                             │
├──────────────────────────────────────────────────────────────────────────────┤
│ Repository: zustand (50 files, 3,200 LOC)                                    │
│ Date: 2026-01-XX                                                             │
│ Model: Claude Opus 4.5                                                       │
├─────────────────┬──────────────┬──────────────┬──────────────┬──────────────┤
│ Metric          │ Baseline     │ FMM Manifest │ FMM MCP      │ Savings      │
├─────────────────┼──────────────┼──────────────┼──────────────┼──────────────┤
│ Lines Read      │ 2,450 ± 320  │ 180 ± 45     │ 165 ± 40     │ 92-93%       │
│ Files Accessed  │ 12 ± 3       │ 4 ± 1        │ 3 ± 1        │ 67-75%       │
│ Read Calls      │ 15 ± 4       │ 8 ± 2        │ 6 ± 2        │ 47-60%       │
│ Tokens Used     │ 24,500       │ 2,940        │ 2,680        │ 88-89%       │
│ Accuracy        │ 94% ± 5%     │ 96% ± 3%     │ 97% ± 2%     │ +2-3%        │
│ Time (ms)       │ 8,200 ± 800  │ 4,100 ± 400  │ 3,800 ± 350  │ 50-54%       │
└─────────────────┴──────────────┴──────────────┴──────────────┴──────────────┘
```

### Per-Task Breakdown

```json
{
  "task_id": "zustand-find-create",
  "scenario": "find_export",
  "results": {
    "baseline": {
      "runs": [
        {"lines_read": 890, "files": 8, "accuracy": 1.0, "time_ms": 3200},
        {"lines_read": 1045, "files": 10, "accuracy": 1.0, "time_ms": 3800},
        {"lines_read": 920, "files": 9, "accuracy": 1.0, "time_ms": 3400}
      ],
      "mean": {"lines_read": 952, "files": 9, "accuracy": 1.0, "time_ms": 3467}
    },
    "manifest": {
      "runs": [
        {"lines_read": 45, "files": 2, "accuracy": 1.0, "time_ms": 1200},
        {"lines_read": 52, "files": 2, "accuracy": 1.0, "time_ms": 1350},
        {"lines_read": 48, "files": 2, "accuracy": 1.0, "time_ms": 1280}
      ],
      "mean": {"lines_read": 48, "files": 2, "accuracy": 1.0, "time_ms": 1277}
    },
    "savings": {
      "lines_read": "95%",
      "files": "78%",
      "time": "63%"
    }
  }
}
```

### Aggregate Report

```
══════════════════════════════════════════════════════════════════
                    FMM BENCHMARK AGGREGATE RESULTS
══════════════════════════════════════════════════════════════════

OVERALL TOKEN SAVINGS
─────────────────────
  Small repo (zustand):     89% reduction (24.5K → 2.7K tokens/task)
  Medium repo (hono):       93% reduction (45.2K → 3.2K tokens/task)
  Large repo (excalidraw):  96% reduction (128K → 5.1K tokens/task)

  AVERAGE: 93% token reduction

ACCURACY COMPARISON
───────────────────
  Baseline accuracy:  92% ± 6%
  FMM Manifest:       95% ± 4%
  FMM MCP:           96% ± 3%

  FINDING: FMM improves accuracy (fewer wrong turns = fewer errors)

CROSSOVER ANALYSIS
──────────────────
  FMM overhead per file: ~15 lines manifest entry
  Break-even point: 3+ files in lookup (nearly always)

  Small codebase (<50 files): 85% savings
  Medium codebase (50-500):   93% savings
  Large codebase (500+):      96% savings

COST PROJECTION (at 1000 queries/day)
─────────────────────────────────────
  Without FMM: $15.00/day (Claude input pricing)
  With FMM:    $1.05/day
  Annual savings: $5,089

══════════════════════════════════════════════════════════════════
```

---

## Implementation Checklist

### Phase 1: Setup (Day 1)
- [ ] Clone test repositories
- [ ] Run fmm generate on each
- [ ] Verify manifests are valid
- [ ] Count baseline file/line statistics
- [ ] Document actual repo sizes

### Phase 2: Task Definition (Day 2)
- [ ] Define 5 tasks per repo (15 total)
- [ ] Establish ground truth for each task
- [ ] Write task JSON files
- [ ] Peer review task definitions

### Phase 3: Instrumentation (Day 3-4)
- [ ] Implement benchmark runner
- [ ] Add token counting
- [ ] Add accuracy grading
- [ ] Test with single task/condition

### Phase 4: Execution (Day 5)
- [ ] Run all benchmarks (45 task-conditions x 3 runs = 135 runs)
- [ ] Capture raw outputs
- [ ] Monitor for failures/anomalies

### Phase 5: Analysis (Day 6-7)
- [ ] Aggregate results
- [ ] Generate summary tables
- [ ] Create visualizations
- [ ] Write findings report

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| LLM behavior varies between runs | High variance in results | 3+ runs per condition, report variance |
| Repos change between benchmark runs | Non-reproducible | Pin to specific commit SHA |
| fmm fails on some files | Incomplete manifest | Handle errors gracefully, report coverage |
| Tasks too easy/hard | Ceiling/floor effects | Calibrate task difficulty across range |
| Model updates during benchmark | Inconsistent baseline | Complete all runs in short window |

---

## Success Criteria

The benchmark demonstrates fmm value if:

1. **Token Reduction:** >= 85% fewer tokens on average across all repos
2. **Accuracy Maintained:** No statistically significant accuracy drop
3. **Consistent Savings:** Reduction holds across all task types
4. **Scalability:** Larger repos show greater savings
5. **Practical Tasks:** Real developer workflows benefit

---

## Appendix: Raw Data Templates

### File Statistics Collection

```bash
# Run this on each repo to establish baseline
for repo in zustand hono excalidraw; do
  echo "=== $repo ==="
  echo "TypeScript files:"
  find benchmark/repos/$repo -name '*.ts' -o -name '*.tsx' | wc -l
  echo "Total lines:"
  find benchmark/repos/$repo -name '*.ts' -o -name '*.tsx' -exec wc -l {} + | tail -1
  echo "Manifest size:"
  wc -l benchmark/repos/$repo/.fmm/manifest.json
  echo ""
done
```

### Token Counting Reference

```typescript
// Using Anthropic's token counting (when available)
// Or tiktoken for OpenAI-compatible counting

import { countTokens } from './token-utils';

const MODELS = {
  'claude-opus-4-5-20251101': {
    context_window: 200_000,
    input_price_per_1m: 15.00,
    output_price_per_1m: 75.00,
  }
};

function calculateCost(inputTokens: number, outputTokens: number, model: string) {
  const pricing = MODELS[model];
  return (inputTokens / 1_000_000 * pricing.input_price_per_1m) +
         (outputTokens / 1_000_000 * pricing.output_price_per_1m);
}
```

---

*Document created: 2026-01-28*
*Author: Stuart Robinson with Claude Opus 4.5*
*Status: Ready for implementation*
