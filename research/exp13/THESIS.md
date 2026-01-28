# The FMM Thesis

## The Core Insight

**LLMs are the devs now. Humans cannot compete at scale.**

Every codebase interaction - reading, understanding, modifying - is increasingly done by LLMs. The economics are clear: LLM tokens cost money, and codebases are large.

## The Problem

LLMs waste tokens reading entire files to understand what they do.

```
grep "validateUser" → 10 matches
  → read file 1 (400 lines) - wrong one
  → read file 2 (600 lines) - wrong one
  → read file 3 (200 lines) - this is it

Total: 1,200 lines = 1,200+ tokens wasted
```

## The Solution

**Manifest JSON.** One file that describes the entire codebase.

```json
{
  "src/auth.ts": {
    "exports": ["validateUser", "createSession"],
    "imports": ["crypto", "./database"],
    "loc": 234
  },
  "src/database.ts": {
    "exports": ["query", "connect"],
    "imports": ["pg"],
    "loc": 156
  }
}
```

## The New Workflow

```
LLM reads .fmm/index.json (one read, entire codebase structure)
  → "validateUser is in src/auth.ts"
  → read src/auth.ts (200 lines)

Total: ~250 lines
Savings: 80%+
```

## The Evidence

| Task | Without FMM | With FMM | Savings |
|------|-------------|----------|---------|
| Review changes | 1,824 lines | 65 lines | 96% |
| Refactor analysis | 2,800 lines | 345 lines | 88% |
| Architecture exploration | 7,135 lines | 180 lines | 97.5% |

## The Economics

- **Per query:** 88-97% fewer tokens
- **Per codebase:** Manifest generated once, used thousands of times
- **At scale:** Massive reduction in LLM compute costs

## Why Manifest, Not Inline Comments

We tried inline frontmatter first:

```typescript
// ---
// file: ./auth.ts
// exports: [validateUser, createSession]
// ---
```

**Problem:** LLMs skip comments. They're trained to find "real code."

Frontmatter in comments = invisible to LLMs.
Frontmatter in JSON manifest = queryable by LLMs.

## The Adoption Path

1. `fmm generate` creates `.fmm/index.json` from any codebase
2. LLMs query manifest before reading files
3. Token costs drop. Everyone wins.

No behavior change required in LLM tools. Just a better data format.

## The Bet

Every codebase with a manifest = cheaper to work with.
Every LLM that queries manifests first = cheaper to run.

**LLMs are the target user. Build for them.**

---

*fmm: Frontmatter Matters*
