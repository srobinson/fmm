# FMM Dogfooding Results: mdcontext Codebase

## Overview

**Target codebase:** [mdcontext](https://github.com/srobinson/mdcontext) — a token-efficient markdown analysis tool for LLM consumption.

**Codebase stats:**
- 123 TypeScript files (120 in `src/`, 3 in `tests/`)
- 39,725 total lines of code
- ~1.22 MB source code
- ~303,826 estimated tokens (chars/4)
- 16 subdirectories in `src/`

**FMM version:** 0.1.0 (tree-sitter based, Rust CLI)

---

## Methodology

### Test Files

Five representative files selected to span the complexity spectrum:

| # | Role | File | Lines | Chars | Est. Tokens |
|---|------|------|------:|------:|------------:|
| 1 | Small utility | `src/search/path-matcher.ts` | 33 | 1,333 | ~333 |
| 2 | Type definitions | `src/embeddings/types.ts` | 359 | 12,027 | ~3,007 |
| 3 | Core module | `src/embeddings/semantic-search.ts` | 1,270 | 40,062 | ~10,016 |
| 4 | CLI command | `src/cli/commands/search.ts` | 1,281 | 42,605 | ~10,651 |
| 5 | Test file | `src/errors/errors.test.ts` | 845 | 27,202 | ~6,801 |

### Questions Asked Per File

For each file, the question asked is: **"What does this file do?"**

This is the most common LLM navigation question and represents the primary use case for FMM.

### Measurement Approach

Since we cannot instrument Claude Code's internal token counter from within a session, we measure:

1. **Input tokens consumed** — estimated from character count of file content that must be read to answer the question (chars ÷ 4)
2. **Navigation overhead** — number of tool calls / files read to locate and understand the file
3. **Qualitative accuracy** — does the answer correctly describe the file's purpose, exports, and dependencies?

---

## Baseline: Without Frontmatter (ALP-311)

### Workflow: "What does this file do?"

**Without FMM**, an LLM must:
1. Read the entire file to understand its purpose
2. Scan imports to understand dependencies
3. Scan exports to understand the public API
4. Parse code structure to summarize functionality

There is no shortcut — the entire file must be consumed.

### Baseline Token Costs (Input)

| File | Lines | Chars | Est. Input Tokens | Notes |
|------|------:|------:|------------------:|-------|
| `path-matcher.ts` | 33 | 1,333 | **333** | Trivial — single function, JSDoc explains it |
| `types.ts` | 359 | 12,027 | **3,007** | All type definitions, must read all to list exports |
| `semantic-search.ts` | 1,270 | 40,062 | **10,016** | Large module, 8 exported functions/types |
| `search.ts` (CLI) | 1,281 | 42,605 | **10,651** | Largest file, complex CLI command wiring |
| `errors.test.ts` | 845 | 27,202 | **6,801** | Repetitive test structure |
| **TOTAL** | **3,788** | **123,229** | **~30,808** | |

### Baseline Navigation Overhead

To answer "what does `semantic-search.ts` do?" without FMM:
- **Minimum tool calls:** 1 (read the file) — if you already know the path
- **Typical tool calls:** 2-4 (search for the file, read it, possibly read dependencies)
- **Full understanding:** 5-8 (read file + read imported modules to understand context)

### Baseline Codebase-Level Navigation

To answer "where is the search functionality implemented?" without FMM:
- Must glob/grep across all 123 files
- Read multiple candidates to find the right ones
- Estimated: 3-5 tool calls, consuming 5,000-15,000 tokens of file content

---

## Post-Frontmatter: With FMM (ALP-313)

### FMM Generation Results

- **Files processed:** 123 (120 src + 3 tests)
- **Manifest size:** 88,977 bytes (~22,244 tokens)
- **Generation time:** < 1 second (Rust + tree-sitter, parallel)
- **No parsing errors** encountered

### Workflow: "What does this file do?" — With FMM

**With FMM**, an LLM can:
1. Read only the FMM header block (6-8 lines) to get: file path, exports, imports, dependencies, LOC
2. This is sufficient to answer "what does this file do?" for most cases
3. Only read the full file if deeper implementation details are needed

### Post-FMM Token Costs (Input)

| File | Full File Chars | FMM Header Chars | FMM Header Lines | FMM Tokens | Reduction |
|------|----------------:|-----------------:|-----------------:|-----------:|----------:|
| `path-matcher.ts` | 1,487 | 153 | 6 | **~38** | **89.8%** |
| `types.ts` | 12,529 | 501 | 6 | **~125** | **95.8%** |
| `semantic-search.ts` | 40,760 | 697 | 8 | **~174** | **98.3%** |
| `search.ts` (CLI) | 43,199 | 593 | 8 | **~148** | **98.6%** |
| `errors.test.ts` | 27,416 | 213 | 7 | **~53** | **99.2%** |
| **TOTAL** | **125,391** | **2,157** | **35** | **~539** | **98.2%** |

### Comparison: Before vs After

| Metric | Without FMM | With FMM | Improvement |
|--------|------------:|---------:|------------:|
| Tokens to answer "what does this file do?" (5 files) | ~30,808 | ~539 | **98.2% reduction** |
| Tool calls to answer | 1 per file (read entire file) | 1 per file (read header only) | Same count, but **57x less data** |
| Tokens for manifest-based lookup | N/A | ~22,244 (one-time) | Amortized across all queries |

### Key Insight: Manifest vs Inline

Two complementary approaches:

| Approach | Best For | Token Cost |
|----------|----------|-----------|
| **Inline FMM header** | "What does THIS file do?" | Read 6-8 lines (~40-175 tokens) |
| **Manifest `exportIndex`** | "Where is X defined?" | Read manifest once (~22K tokens), then O(1) lookups |

For a codebase of 123 files:
- Reading ALL files: ~303,826 tokens
- Reading ALL FMM headers: ~5,500 tokens (estimated)
- Reading manifest once: ~22,244 tokens

The manifest is more efficient for "where is X?" queries across the codebase.
The inline headers are more efficient for understanding individual files.

---

## Frontmatter Accuracy Analysis (ALP-314)

### Verified Files

Each of the 5 test files was manually verified against the actual source code.

#### 1. `path-matcher.ts` — ACCURATE

```
// exports: [matchPath]
```
- Correct: file exports exactly one function `matchPath`
- No imports/dependencies (correctly omitted)
- LOC 33 correct

#### 2. `types.ts` — ACCURATE

```
// exports: [BatchProgress, ContextLine, EmbedOptions, EmbeddingProvider, EmbeddingProviderWithMetadata,
//           EmbeddingResult, HnswIndexParams, QUALITY_EF_SEARCH, SemanticSearchOptions,
//           SemanticSearchResult, SemanticSearchResultWithStats, VectorEntry, VectorIndex,
//           calculateFileImportanceBoost, calculateHeadingBoost, calculateRankingBoost,
//           hasProviderMetadata, preprocessQuery]
```
- Correct: captures all 18 exports (interfaces, types, constants, and functions)
- No external imports (correctly omitted — pure type file with only local constants)
- LOC 359 correct

#### 3. `semantic-search.ts` — ACCURATE

```
// exports: [BuildEmbeddingsOptions, BuildEmbeddingsResult, DirectoryEstimate, EmbeddingBatchProgress,
//           EmbeddingEstimate, EmbeddingStats, FileProgress, buildEmbeddings, checkPricingFreshness,
//           estimateEmbeddingCost, getEmbeddingStats, getPricingDate, semanticSearch,
//           semanticSearchWithContent, semanticSearchWithStats]
// imports: [node:fs/promises, node:path, effect]
// dependencies: [../errors/index.js, ../index/storage.js, ../index/types.js, ./embedding-namespace.js,
//                ./hyde.js, ./openai-provider.js, ./provider-factory.js, ./types.js, ./vector-store.js]
```
- Correct: 15 exports captured (all interfaces and functions)
- Imports correctly split between external (node:*, effect) and local dependencies
- Re-exports (`checkPricingFreshness`, `getPricingDate`) correctly detected
- LOC 1270 correct

#### 4. `search.ts` (CLI) — ACCURATE

```
// exports: [searchCommand]
// imports: [node:fs/promises, node:path, node:readline, @effect/cli, effect]
// dependencies: [../../config/index.js, ../../embeddings/semantic-search.js, ../../embeddings/types.js,
//                ../../index/storage.js, ../../index/types.js, ../../search/cross-encoder.js,
//                ../../search/hybrid-search.js, ../../search/query-parser.js, ../../search/searcher.js,
//                ../../summarization/index.js, ../options.js, ../shared-error-handling.js, ../utils.js]
```
- Correct: only 1 public export (`searchCommand`) despite being 1,281 lines
- 13 local dependencies accurately captured — excellent for understanding the file's integration points
- This is a prime example of FMM's value: 1,281 lines reduced to "exports searchCommand, depends on 13 modules"
- LOC 1281 correct

#### 5. `errors.test.ts` — ACCURATE

```
// imports: [effect, vitest]
// dependencies: [../cli/error-handler.js, ./index.js]
```
- Correct: no exports (test files shouldn't export)
- Correctly identifies it depends on the error handler and error index
- LOC 845 correct

### Accuracy Summary

| File | Exports | Imports | Dependencies | LOC | Overall |
|------|---------|---------|-------------|-----|---------|
| `path-matcher.ts` | Correct | Correct | Correct | Correct | **100%** |
| `types.ts` | Correct | Correct | Correct | Correct | **100%** |
| `semantic-search.ts` | Correct | Correct | Correct | Correct | **100%** |
| `search.ts` (CLI) | Correct | Correct | Correct | Correct | **100%** |
| `errors.test.ts` | Correct | Correct | Correct | Correct | **100%** |

**Overall accuracy: 100% across all 5 test files.**

Tree-sitter AST parsing produces reliable results for TypeScript.

### Issues Found

1. **Absolute paths in `file:` field** — The `file:` line contains the full absolute path (e.g., `/Users/alphab/Dev/LLM/DEV/mdcontext/src/search/path-matcher.ts`) instead of a relative path. This is not portable and wastes tokens. **Recommendation: use relative paths from project root.**

2. **Manifest saved to CWD, not target project** — When running `fmm generate /path/to/project/`, the `.fmm/index.json` manifest is saved to the current working directory, not the target project directory. This is confusing for external projects. **Recommendation: save manifest in the target directory.**

---

## Navigation Workflow Tests (ALP-315)

### Test 1: "Where is the search functionality implemented?"

**Without FMM:** Must glob for `*search*` files, then read each to understand which is the main search module. Estimated: 3-5 tool calls, reading several files.

**With FMM manifest:** Read `.fmm/index.json`, look at `exportIndex` for search-related exports:
- `semanticSearch` → `src/embeddings/semantic-search.ts`
- `searchCommand` → `src/cli/commands/search.ts`
- `search`, `searchContent` → `src/search/searcher.ts`
- `hybridSearch` → `src/search/hybrid-search.ts`

**Result:** 1 tool call (read manifest) vs 3-5 tool calls. **2-5x fewer tool calls.**

### Test 2: "Where is `EmbeddingProvider` defined?"

**Without FMM:** Grep for `EmbeddingProvider` across all files. Estimated: 1-2 tool calls (grep + read).

**With FMM manifest:** Check `exportIndex.EmbeddingProvider` → `src/embeddings/types.ts`. **1 lookup, 0 file reads.**

**Result:** Instant from manifest. No file I/O needed.

### Test 3: "What modules does `search.ts` CLI command depend on?"

**Without FMM:** Read the entire 1,281-line file to find all imports. Cost: ~10,651 tokens.

**With FMM header:** Read first 8 lines: `dependencies: [13 modules listed]`. Cost: ~148 tokens.

**Result:** **98.6% token reduction.** Same answer, 72x less input.

### Test 4: "What does the errors test file cover?"

**Without FMM:** Read all 845 lines to understand test coverage. Cost: ~6,801 tokens.

**With FMM header:** Read first 7 lines: `imports: [effect, vitest], dependencies: [../cli/error-handler.js, ./index.js]`. Cost: ~53 tokens. Combined with the existing JSDoc at the top of the file, the FMM block tells you: "this tests the error types from `./index.js` and error formatting from `../cli/error-handler.js`."

**Result:** **99.2% token reduction** for a high-level answer. Full file read still needed for specific test case details.

### Test 5: "Give me an overview of all exports in the embeddings module"

**Without FMM:** Read every file in `src/embeddings/` (23 files, likely 5,000+ lines). Estimated: 23 file reads.

**With FMM manifest:** Query manifest for all files under `src/embeddings/` path. One read, complete export listing across all 23 files.

**Result:** **1 tool call vs 23.** Order-of-magnitude improvement.

### Navigation Summary

| Workflow | Without FMM | With FMM | Improvement |
|----------|-------------|----------|-------------|
| Find feature location | 3-5 grep/reads | 1 manifest read | **3-5x fewer calls** |
| Find export definition | 1-2 grep/reads | 1 manifest lookup | **Instant O(1)** |
| Understand file deps | Read full file | Read 8-line header | **72x less data** |
| Module overview | Read N files | 1 manifest query | **N× fewer reads** |
| "What does file do?" | Read full file | Read header | **57x less data** |

---

## Summary & Recommendations (ALP-316, ALP-317)

### Key Findings

1. **98.2% token reduction** for "what does this file do?" across the 5 test files
2. **100% accuracy** in generated frontmatter (exports, imports, dependencies, LOC)
3. **Manifest provides O(1) export lookups** — eliminates grep/glob for "where is X?"
4. **< 1 second** to generate frontmatter for 123 files (Rust + tree-sitter + rayon)

### Token Economics

For the mdcontext codebase (123 files, ~304K tokens):

| Scenario | Token Cost | Notes |
|----------|-----------|-------|
| Read entire codebase | ~303,826 | Without any FMM support |
| Read manifest once | ~22,244 | One-time cost, covers all files |
| Read all FMM headers | ~5,500 est. | If scanning all files with headers |
| Answer "what does file X do?" (avg) | ~108 tokens | Just the FMM header |
| Answer "what does file X do?" (no FMM) | ~6,162 tokens | Must read whole file |

**ROI:** The manifest costs ~22K tokens to read once, then every subsequent query saves thousands of tokens. Break-even after ~4 file lookups.

### Format Issues to Fix (ALP-317)

1. **Use relative paths in `file:` field** — Current: absolute paths waste tokens and aren't portable. Fix: resolve relative to project root (or `.fmmrc.json` location).

2. **Save manifest in target project directory** — Current: saves to CWD. Fix: save `.fmm/index.json` in the directory being processed.

3. **Consider adding a `purpose:` field** — A one-line description would make the frontmatter even more useful for "what does this file do?" without reading any code. Could be AI-generated or manually authored.

4. **Consider `type:` field for test files** — Marking files as `test`, `config`, `types`, `cli` etc. would help LLMs categorize files without reading them.

### Recommendations

- **Ship the current format** — It works. 100% accuracy, massive token savings.
- **Fix the path issues** (relative paths + manifest location) before v1.0
- **Add `.fmm/index.json` to `.gitignore` template** — The manifest is generated, not authored
- **Document the "read manifest first" pattern** in CLAUDE.md integration guide
- **Consider a `--watch` mode** for development — auto-update frontmatter on file changes
