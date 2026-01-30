# Case Study: Fixing a Real Bug in a 9,000-File Repo

**Date:** 2026-01-30
**Repo:** [ruvnet/claude-flow](https://github.com/ruvnet/claude-flow) (9,008 files)
**Issue:** [#1044 — Model selected in Claude Code is not used](https://github.com/ruvnet/claude-flow/issues/1044)
**PR:** [#1050](https://github.com/ruvnet/claude-flow/pull/1050)

---

## The Bug

A user reported that claude-flow ignores your Claude Code model setting. You pick Sonnet 4.5 — it uses Opus 4.5 anyway.

## What Happened

```
$ git clone ruvnet/claude-flow    # 9,008 files
$ fmm init                        # 3 seconds
✓ Generated .fmm/index.json (2,221 source files indexed)
```

One command. 2,221 files mapped — exports, imports, dependencies, LOC. The manifest told us exactly where model configuration lives without reading a single source file.

### Without fmm

You'd grep 9,000 files for "model", "opus", "sonnet" — hundreds of matches across docs, configs, tests, node_modules. You'd open file after file trying to trace the model selection flow. Typical cost: **40-60 tool calls, 50K+ tokens**.

### With fmm

The manifest mapped the codebase structure. We navigated straight to 2 files:

```
v3/@claude-flow/cli/src/services/headless-worker-executor.ts
v3/@claude-flow/cli/src/runtime/headless.ts
```

**3 root causes found. 5-line fix. 2 files changed.**

## The Fix

### 1. Wrong model ID (line 275)

```diff
 const MODEL_IDS: Record<ModelType, string> = {
   sonnet: 'claude-sonnet-4-20250514',
-  opus: 'claude-opus-4-20250514',
+  opus: 'claude-opus-4-5-20251101',
   haiku: 'claude-haiku-4-20250514',
 };
```

The Opus model ID didn't even exist. Every Opus worker was silently failing or falling back.

### 2. Environment variable stomping (line 1122)

```diff
-  // Set model
-  env.ANTHROPIC_MODEL = MODEL_IDS[options.model];
+  // Set model — only override if not already set by user
+  if (!env.ANTHROPIC_MODEL) {
+    env.ANTHROPIC_MODEL = MODEL_IDS[options.model];
+  }
```

The user's model choice was unconditionally overwritten.

### 3. Hardcoded sonnet override (line 145)

```diff
   const result = await executor.execute(workerType, {
     timeoutMs: timeout,
-    model: 'sonnet',
     sandbox: 'permissive'
   });
```

The runtime forced `model: 'sonnet'` on every worker, stomping each worker's own configured model.

## The Workflow

```
fmm init              → index the codebase (2,221 files in 3s)
navigate via manifest  → find the 2 files that matter
apply fix             → 5 lines across 2 files
git clean -fd .fmm    → strip fmm artifacts
git push              → clean PR, no fmm in the diff
```

fmm is a development tool. It helps you understand code. It doesn't ship with your code.

## Numbers

| Metric | Without fmm (estimate) | With fmm |
|--------|----------------------|----------|
| Files in repo | 9,008 | 9,008 |
| Files indexed | — | 2,221 |
| Files read to find bug | ~30-50 | 2 |
| Tool calls | ~40-60 | ~10 |
| Time to root cause | minutes of searching | seconds of navigating |
| Fix size | same | 5 lines, 2 files |
