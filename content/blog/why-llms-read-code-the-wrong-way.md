# Why LLMs Read Code the Wrong Way (And What We Found When We Stopped Guessing)

**tl;dr** We ran 60+ controlled experiments to figure out how LLMs actually navigate codebases. The short version: they ignore your comments, never check hidden directories, and Grep-then-read-everything is their only play. Sidecar metadata files cut token usage by 88-97% — but only when you explicitly tell the LLM they exist. The discovery story behind fmm.

---

## We started with the obvious idea

Every developer who has watched an LLM chew through a codebase has had the same thought: *what if we just told it what was in each file?*

The idea behind fmm (Frontmatter Matters) started there. We wanted to give LLMs structured metadata about source files — exports, imports, dependencies, line counts — so they could navigate intelligently instead of reading every file top to bottom.

Our first approach was inline frontmatter. A YAML block at the top of each source file, wrapped in comments:

```typescript
// ---
// fmm: v0.2
// exports: [createSession, validateSession, destroySession]
// imports: [jwt, redis-client]
// dependencies: [./types, ./config]
// loc: 234
// ---

import { sign, verify } from 'jwt';
import { client } from './redis-client';
// ... rest of the file
```

Clean. Non-intrusive. Any developer reading the file gets a quick summary. And surely an LLM, reading the same file, would notice structured metadata sitting right at the top and use it to skip reading other files.

We were wrong.

## Experiment 14: Zero out of twelve

We built an 18-file TypeScript authentication app — realistic enough to require navigation across modules, small enough to control variables. Then we ran 12 Claude sessions across four conditions:

| Condition | What we changed |
|---|---|
| **Control** | Clean codebase, no metadata |
| **Inline** | FMM comments at the top of every file |
| **Manifest** | `.fmm/index.json` containing full project metadata |
| **Hint** | Manifest + system prompt mentioning its existence |

Three runs per condition. Same task each time. We watched every session.

Discovery rate across all conditions: **0 out of 12**.

Not one session — across any condition — organically discovered or used the metadata. Not inline comments. Not the manifest file. Not even with a system prompt hint.

Here is what the numbers looked like:

| Condition | Avg Tool Calls | Avg Files Read | Avg Tokens | Avg Cost | Accuracy |
|---|---|---|---|---|---|
| Control | 13.3 | 11.3 | 121,438 | $0.062 | 92-96% |
| Inline | 14.3 | 10.3 | 150,486 | $0.068 | 92-96% |
| Manifest | 14.0 | 10.0 | 134,367 | $0.061 | 92-96% |
| Hint | 15.7 | 11.3 | 168,848 | $0.079 | 92-96% |

Look at the inline condition. More tool calls than control. More tokens. Higher cost. The metadata was sitting right there in every file the LLM opened, and it made things *worse* because the files were longer.

The hint condition — where we literally told the model a metadata index existed — was the most expensive of all. The LLM acknowledged the hint and then proceeded to Grep and read files exactly the same way it always does.

Accuracy was identical across all four conditions. The metadata provided zero navigational advantage when the LLM did not know how to use it.

## The default algorithm is hard-wired

Watching those 12 sessions revealed a pattern so consistent it might as well be firmware. Every single session followed the same strategy:

1. **Grep** for the target symbol or keyword
2. **Read the entire file** that matched
3. **Grep again** for related symbols found in that file
4. **Read more entire files**
5. **Summarize** from everything consumed

No LLM session ever paused to think "maybe there's a project index I should check first." No session noticed the YAML block at the top of a file and thought "I can use this to skip reading other files." The comments were processed as part of the file content and immediately forgotten.

This is the core insight: **LLMs treat code comments as noise.** They are trained on millions of codebases where comments are unreliable, outdated, or irrelevant. The model has learned, correctly, that the actual code is the source of truth. So it reads the actual code. Every time. All of it.

Hidden directories fare even worse. The `.fmm/` directory with a complete project manifest was invisible. LLMs do not speculatively list hidden directories. They do not check for metadata indexes. They have no reason to — nothing in their training suggests that `.fmm/index.json` contains anything useful.

## The pivot: sidecar files and explicit instructions

The Exp14 results killed inline frontmatter as a strategy. But they also showed us exactly what would work.

If LLMs won't discover metadata organically, we need two things:

1. **Metadata in a format LLMs naturally query** — not comments inside files, but standalone files that show up in directory listings and Grep results.
2. **An explicit instruction to look for it** — delivered through whatever mechanism the LLM checks before starting work.

This became the sidecar model. Every source file gets a companion `.fmm` file:

```
src/auth/session.ts       (source — 234 lines)
src/auth/session.ts.fmm   (sidecar — structured metadata)
```

The sidecar contains the same YAML we tried inline, but as a standalone file:

```yaml
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis-client]
dependencies: [./types, ./config]
loc: 234
```

And the critical second piece — a `CLAUDE.md` instruction:

```
When navigating code, check for .fmm sidecar files first.
Read sidecars to understand file structure before opening source files.
```

Two sentences. That is the entire behavior change.

## Experiment 13: The reduction is real

With sidecars and explicit instructions in place, we tested on a serious codebase: agentic-flow, a 244-file TypeScript project with 81,732 lines of code.

We ran three task types and measured how many lines the LLM needed to read:

| Task | Without fmm | With fmm | Reduction |
|---|---|---|---|
| Code review | 1,824 lines | 65 lines | **96.4%** |
| Refactor analysis | 2,800 lines | 345 lines | **87.7%** |
| Architecture exploration | 7,135 lines | 180 lines | **97.5%** |

Architecture exploration went from reading 7,135 lines to 180. The LLM read sidecar files, built a mental map of the project, identified the relevant modules, and only opened the three source files it actually needed.

This is the 88-97% token reduction range we now cite, and it comes from Exp13 on a real, production-scale codebase — not a toy example.

## Experiment 15: How you deliver the instruction matters

Knowing that explicit instruction is required, we tested how to deliver it. Exp15 ran 48 sessions (4 conditions, 4 tasks, 3 runs each) on a 1,306-file codebase.

The two conditions that mattered:

| Delivery Mechanism | Avg Tool Calls | Avg Cost |
|---|---|---|
| CLAUDE.md only | 22.2 | $0.55 |
| Skill + MCP server | 15.5 | $0.41 |

The Skill+MCP approach — where fmm registers as a tool the LLM can call directly — produced 30% fewer tool calls and 25% lower cost compared to CLAUDE.md instructions alone.

The difference makes sense. CLAUDE.md tells the LLM *what to do* but leaves execution to its default tool-calling patterns. An MCP server gives the LLM a dedicated `fmm_query` tool that returns structured results. Instead of "Grep for .fmm files, read them, parse the YAML, figure out what to do," the LLM calls one tool and gets back exactly the metadata it needs.

But here is the thing — CLAUDE.md alone still works. A single instruction in CLAUDE.md transforms the LLM's very first action from "let me Grep for the symbol" to "let me check if there's an FMM index." The behavior change is immediate and consistent. MCP makes it more efficient, but CLAUDE.md makes it possible.

## What this means if you are building for LLMs

Three takeaways from 60+ experimental runs:

**1. Do not put metadata in comments.**
LLMs will not use it. Inline metadata increases token consumption without improving outcomes. This is not a theoretical concern — we measured it (Exp14). Comments are invisible to LLM reasoning.

**2. Structured data in queryable locations beats everything.**
Sidecar files, manifest indexes, dedicated API endpoints. Anything the LLM can find through its normal tool-calling patterns (file listing, Grep, dedicated tools) will get used. Anything that requires the LLM to "notice" something will not.

**3. Explicit instruction is non-negotiable.**
No amount of clever file placement triggers organic discovery. You must tell the LLM — through CLAUDE.md, system prompts, MCP tool registration, or equivalent — that the metadata exists and how to use it. Two sentences is enough. Zero sentences means zero adoption.

The broader lesson is uncomfortable for anyone building developer tools with AI in mind: **design for observed behavior, not expected behavior.** We expected LLMs to notice structured comments. We expected them to explore hidden directories. We expected a system prompt hint to change their strategy. None of that happened.

What happened is that LLMs followed their Grep-read-summarize loop until we gave them a different loop to follow — and then they followed the new one immediately and consistently.

The gap between "should work" and "does work" is where most AI-augmented tooling fails. We found that gap at session zero of Exp14, and everything fmm became after that point was built on the evidence from the other side.

---

*fmm is open source. The experiment data referenced in this post (Exp13, Exp14, Exp15) is available in the project repository.*
