# Social Content — claude-flow #1044

## Tweet Thread

### Tweet 1 (Hook)

Someone reported a bug in a 9,000-file repo.

"Model selected in Claude Code is not used"

I fixed it in 5 lines. Across 2 files. In a codebase I'd never seen before.

Here's how fmm changes the game. 🧵

### Tweet 2 (The Problem)

claude-flow (github.com/ruvnet/claude-flow) ignores your model selection. You pick Sonnet — it uses Opus anyway.

9,008 files. Where do you even start?

grep "model" → hundreds of matches
grep "opus" → scattered across docs, tests, configs

### Tweet 3 (fmm)

```
$ fmm init
✓ Generated .fmm/index.json (2,221 files indexed)
```

3 seconds. Every source file mapped — exports, imports, dependencies.

No grepping. No guessing. The manifest tells you exactly where model configuration lives.

### Tweet 4 (The Fix)

3 root causes. All found by navigating the manifest:

1. Wrong Opus model ID (doesn't exist)
2. Env var stomping user's choice
3. Runtime hardcoding 'sonnet' on every worker

5 lines changed. 2 files. PR merged.

github.com/ruvnet/claude-flow/pull/1050

### Tweet 5 (The Point)

fmm is infrastructure for LLM-assisted development.

Before: LLMs grep your codebase, read dozens of files, burn tokens guessing.

After: LLMs query a manifest, read only what matters.

88-97% fewer tokens. Same answers.

github.com/srobinson/fmm

---

## Blog Post (Short Form)

### How I Fixed a Bug in a 9,000-File Repo I'd Never Seen Before

There's a project called [claude-flow](https://github.com/ruvnet/claude-flow) — an orchestration framework for Claude Code. Someone [reported](https://github.com/ruvnet/claude-flow/issues/1044) that it ignores your model selection. You pick Sonnet 4.5, it uses Opus 4.5 anyway.

9,008 files. I'd never opened this repo before.

#### The old way

grep for "model", "opus", "sonnet" across thousands of files. Open the top matches. Read them. Realize half are docs or tests. Keep searching. Eventually piece together the model selection flow across 3-4 files.

With an LLM helping? It would burn 40-60 tool calls and 50K+ tokens doing the same grep-read-grep-read loop.

#### The fmm way

```bash
git clone ruvnet/claude-flow
fmm init
```

Three seconds. 2,221 source files indexed — every export, import, dependency chain, mapped into a single manifest an LLM can query.

The manifest showed me exactly where model configuration lives. Two files:

- `headless-worker-executor.ts` — model ID mapping + env var handling
- `headless.ts` — runtime model override

#### Three bugs, one pattern

1. **Wrong model ID.** `claude-opus-4-20250514` doesn't exist. The correct ID is `claude-opus-4-5-20251101`. Every Opus worker was silently broken.

2. **Environment variable stomping.** `ANTHROPIC_MODEL` was unconditionally overwritten before spawning Claude CLI. Your preference? Gone.

3. **Hardcoded override.** The runtime passed `model: 'sonnet'` to every worker, stomping each worker's own configuration.

Five lines changed. [PR #1050](https://github.com/ruvnet/claude-flow/pull/1050).

#### Why this matters

LLMs are the developers now. Every time an LLM navigates your codebase, you pay for it — in tokens, in time, in wrong answers from incomplete context.

fmm generates a structural manifest that gives LLMs a map instead of a flashlight. Read the manifest, find the file, read only what you need.

The difference:
- **Without fmm:** 30-50 files read, 40-60 tool calls, minutes of searching
- **With fmm:** 2 files read, ~10 tool calls, seconds of navigating

Same answer. Fraction of the cost.

And when you're done? `git clean -fd .fmm` — fmm is a dev tool. It doesn't ship with your code.

[github.com/srobinson/fmm](https://github.com/srobinson/fmm)
