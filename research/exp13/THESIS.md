# The FMM Thesis

## The Problem

LLMs waste tokens reading entire files to understand what they do.

```
grep "validateUser" → 10 matches
  → read file 1 (400 lines) - wrong one
  → read file 2 (600 lines) - wrong one
  → read file 3 (200 lines) - this is it

Total: 1,200 lines to find the right context
```

## The Solution

Frontmatter = metadata in the first 10 lines of every file.

```typescript
// ---
// file: ./auth.ts
// exports: [validateUser, createSession]
// imports: [crypto, ./database]
// loc: 234
// ---
```

## The New Workflow

```
grep "validateUser" → 10 matches
  → read first 15 lines of file 1 - exports don't match, skip
  → read first 15 lines of file 2 - exports don't match, skip
  → read first 15 lines of file 3 - exports: [validateUser] ✓
  → read full file 3 (200 lines)

Total: 245 lines
Savings: 80%
```

## The Evidence

| Task | Without FMM | With FMM | Savings |
|------|-------------|----------|---------|
| Review changes | 1,824 lines | 65 lines | 96% |
| Refactor analysis | 2,800 lines | 345 lines | 88% |
| Architecture exploration | 7,135 lines | 180 lines | 97.5% |

## The Economics

- **Users:** Lower API costs
- **Providers:** Less compute
- **Everyone:** Faster responses

## The Adoption Path

1. Codebases add frontmatter (`fmm generate`)
2. LLM tools adopt "peek first" as default

No manifests. No discovery layers. Just a behavior change in the READ step.

## The Bet

Every codebase with frontmatter = cheaper to work with.
Every LLM that peeks first = cheaper to run.

The more codebases have it, the more pressure on tools to optimize for it.

---

*fmm: Frontmatter Matters*
