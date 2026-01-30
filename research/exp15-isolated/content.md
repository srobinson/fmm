# exp15 Distribution Content

---

## Twitter/X Thread

### Tweet 1 (Hook)

Claude Code spends most of its tokens figuring out what files do.

Grep for a function. Read the file. Grep for its imports. Read those files. Repeat.

We cut that loop from 55 tool calls to 3. Here's how.

### Tweet 2 (The insight)

Watch what Claude does when it opens a source file: it reads the first ~20 lines trying to figure out what the file exports and what it depends on.

What if that information was already there? Structured. Machine-readable. In the first 15 lines of every file.

### Tweet 3 (What FMM does)

fmm (Frontmatter Matters) adds a metadata header to every source file:

```
// --- FMM ---
// file: src/core/pipeline.ts
// exports: [createPipeline, PipelineConfig]
// imports: [zod, lodash]
// dependencies: [./engine, ./validators]
// loc: 142
// --- END FMM ---
```

Auto-generated from the AST. The LLM reads 15 lines and knows everything it needs to decide: is this file relevant?

### Tweet 4 (The cost problem)

Without fmm, "describe this project's architecture" on a 1,000-file codebase:

- Glob for files (1 call)
- Read 20-30 files to understand them (20-30 calls)
- Grep for imports to trace connections (10-15 calls)
- Read more files (5-10 calls)

55 tool calls. Thousands of tokens on file contents that are mostly irrelevant.

### Tweet 5 (The result)

With fmm: 3 tool calls.

1. Get the project file map (every file, its exports, LOC — one call)
2. Read FMM headers of the key files (15 lines each, not 300)
3. Done.

Same answer. 95% fewer tool calls. Fraction of the tokens.

### Tweet 6 (The experiment)

We proved this with 36 isolated Docker experiments.

Clean state. No session memory. Same codebase (1,030 files). Same prompts.

Architecture task: 55 avg tool calls → 27
Dependency mapping: 45 → 10
Export lookup: 5 → 1

The LLM makes better decisions when it has structural metadata upfront.

### Tweet 7 (Why it works)

The LLM already tries to understand file structure before reading source. It's doing the work — just inefficiently.

FMM gives it the answer for free. Exports, imports, dependencies. In the exact place it already looks: the top of the file.

Not a new workflow. The same workflow, with the information pre-computed.

### Tweet 8 (Beyond navigation)

This isn't just navigation. It's context efficiency.

Every token spent on "what does this file do?" is a token not spent on "solve the actual problem."

FMM frontmatter headers give the LLM orientation for free. It spends its context window on the work that matters.

### Tweet 9 (The approach)

Three pieces that work together:

1. FMM headers in every file — structural metadata at the point of consumption
2. A pre-built index for O(1) symbol lookup — "where is createPipeline defined?" answered instantly
3. A compact project map — 93KB text file vs 682KB JSON dump. Navigate, don't download.

### Tweet 10 (CTA)

fmm is open source. Rust CLI, MCP server, works with any language.

Index your codebase. Inject frontmatter headers. Watch your LLM stop wasting tokens figuring out what it's looking at.

github.com/srobinson/fmm

---

## Blog Post

### Title

Your LLM Spends Most of Its Tokens Figuring Out What Files Do

### Subtitle

How frontmatter metadata cuts code navigation costs by 90%

### Body

**Watch what an LLM does when you ask it about your codebase.**

Ask Claude Code to describe your project's architecture. Watch the tool calls. It will:

1. Glob for source files
2. Read a file, scan the first 20 lines, try to figure out what it exports
3. Grep for import statements to trace dependencies
4. Read the imported files, repeat step 2
5. Eventually piece together an answer

On a 1,000-file codebase, that's 50+ tool calls. Thousands of tokens consumed reading file contents — most of which turn out to be irrelevant. The LLM is doing structural analysis the hard way: reading source code to extract metadata that could have been pre-computed.

This is the most expensive part of LLM-assisted development, and it happens on every task. Bug investigation? Read files to find the relevant ones. Refactoring? Read files to understand dependencies. Impact analysis? Read files to trace what depends on what.

The LLM isn't bad at this. It's just doing work that doesn't need to happen at inference time.

#### The Insight: Claude Already Looks at the Top of the File

Here's what we noticed: when Claude opens a file, it reads the first ~20 lines. It's looking for exports, imports, class definitions — anything that tells it what this file *is* and whether it's relevant to the task.

It's already doing the right thing. It just doesn't find structured data there. So it reads further, or greps for more context, or opens another file.

What if the answer was already there?

#### FMM: Structural Metadata at the Point of Consumption

fmm (Frontmatter Matters) adds a machine-generated metadata header to every source file:

```typescript
// --- FMM ---
// file: src/core/pipeline.ts
// exports: [createPipeline, PipelineConfig, PipelineError]
// imports: [zod, lodash]
// dependencies: [./engine, ./validators, ../utils/logger]
// loc: 142
// --- END FMM ---

import { z } from 'zod';
import { Engine } from './engine';
// ... rest of the file
```

Auto-generated from the AST. Accurate. Updated on every index run. The LLM reads 15 lines and knows:

- **What this file exports** — every public symbol
- **What packages it uses** — external dependencies
- **What local files it imports from** — the dependency graph
- **How big it is** — lines of code

That's enough to decide: is this file relevant to my task? Should I read further or move on?

No Grep needed. No reading 300 lines to find the export statements buried at the bottom. Fifteen lines, and the LLM has full orientation.

#### The Cost Difference

We ran 36 isolated experiments in Docker to measure the impact. Same codebase (1,030 TypeScript files). Same prompts. Clean state every run — no session memory, no cached context.

**Architecture overview:**
- Without fmm: 55 tool calls average. Glob, Read, Grep, Read, Read...
- With fmm: 27 tool calls. Get the file map, read headers of key files, done.

**Dependency mapping:**
- Without fmm: 45 tool calls. Manual import tracing across dozens of files.
- With fmm: 10 tool calls. Query the index, read relevant headers.

**Export lookup ("where is createPipeline defined?"):**
- Without fmm: 5 tool calls. Grep the codebase, read the match.
- With fmm: 1 tool call. O(1) index lookup.

**Impact analysis ("what breaks if I change this file?"):**
- Without fmm: 8 tool calls. Grep for imports, read each file.
- With fmm: 3 tool calls. Lookup the symbol, get the dependency graph.

Every tool call is tokens — input tokens for the request, output tokens for the response, context tokens for everything that came before. Cutting tool calls by 50-95% is a direct cost reduction. But the bigger win is context efficiency: the LLM's limited context window is spent on the actual problem instead of on orientation.

#### Why This Works Better Than a Centralized Index

The obvious approach is a big JSON index — dump everything into one file and let the LLM query it. We tried that. On a 1,000-file project, the index is 682KB. That overflows Claude's tool result limit. The LLM calls the tool, gets an error, and falls back to Grep. Worse than having no index at all.

fmm takes a different approach: **distributed metadata**. Every file carries its own structural profile. The LLM encounters the metadata naturally as it navigates — the same way a developer reads the top of a file to orient themselves.

For targeted queries, fmm also provides an MCP server with focused tools:
- `fmm_lookup_export(name)` — O(1) symbol-to-file lookup
- `fmm_dependency_graph(file)` — upstream deps + downstream dependents
- `fmm_search(imports: "crypto")` — find files by structural criteria

These return small, focused results. Not a 682KB dump — a direct answer to a specific question.

For project-level discovery, fmm returns a compact file map: directory-grouped files with names, LOC, and top exports. 93KB for a 1,300-file project. Scannable. Fits in context. Gives the LLM enough to decide where to look next.

#### The Principle

Every token spent on "what does this file do?" is a token not spent on solving the actual problem.

LLMs already try to understand file structure before reading source. They look at the top of the file. They grep for imports. They read files to find exports.

Pre-compute that work. Put it where the LLM already looks. Let it make navigation decisions in 15 lines instead of 300.

That's what fmm does. Not a new workflow — the same workflow, with the structural metadata pre-computed and placed at the point of consumption.

---

fmm is open source. Rust CLI, MCP server, works with TypeScript, JavaScript, Python, Rust, Go, Java, C#, Ruby, and C++.

github.com/srobinson/fmm
