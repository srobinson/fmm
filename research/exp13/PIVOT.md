# The Pivot: Comments Are Skipped

**Date:** 2026-01-28
**Status:** Critical insight - changes project direction

---

## The Realization

LLMs don't read full files. They read the first ~20 lines.

**Frontmatter is already in those lines.**

**But it's being skipped because it looks like comments.**

---

## The Problem

```typescript
// ---
// file: ./auth.ts
// exports: [validateUser, createSession]
// imports: [crypto]
// loc: 234
// ---
```

LLM sees this and thinks: "comment block → skip → find real code"

**Comment syntax = noise = invisible**

---

## Why Our Experiments "Worked"

The FMM agent was explicitly told:
> "Read the first 15 lines and USE the frontmatter"

That's not organic behavior. That's instruction-following.

Without the instruction, the LLM reads the same 20 lines but **ignores the comments**.

---

## The Implication

**Frontmatter as comments is dead on arrival.**

No amount of adoption, tooling, or evangelism fixes this. The format itself is invisible to LLMs.

---

## Alternative Formats

### Option 1: Make It Code

```typescript
export const __meta = {
  exports: ["validateUser", "createSession"],
  imports: ["crypto"],
  loc: 234
};
```

- LLM parses this as code
- It's visible, not skipped
- TypeScript-valid, can be type-checked
- **Friction:** Changes file structure, might confuse bundlers

### Option 2: Manifest File

```
.fmm/
  index.json     ← LLM queries this, not file headers
```

```json
{
  "src/auth.ts": {
    "exports": ["validateUser", "createSession"],
    "imports": ["crypto"],
    "loc": 234
  }
}
```

- LLM reads JSON, not comments
- Query before reading files
- No changes to source files
- **Friction:** Separate file to maintain, sync issues

### Option 3: Tool-Level Extraction

```typescript
/* @fmm {"exports":["validateUser"],"loc":234} */
```

The Read tool parses this marker and surfaces metadata separately:

```
FRONTMATTER: {"exports":["validateUser"],"loc":234}
FILE CONTENT:
import { createHash } from 'crypto';
...
```

- Comment in file, but tool extracts it
- LLM sees structured data, not comment
- **Friction:** Requires tool changes

### Option 4: First-Line JSON

```typescript
//@ {"exports":["validateUser"],"loc":234}
import { createHash } from 'crypto';
```

- Single line, obvious pattern
- JSON after special marker
- Tool or LLM can parse
- **Friction:** Non-standard, might break syntax highlighters

---

## Evaluation Matrix

| Approach | LLM Visibility | Dev Friction | Tool Changes | Adoption Path |
|----------|----------------|--------------|--------------|---------------|
| Code export | High | Medium | None | Hard (bundler issues) |
| Manifest file | High | Low | None | Medium |
| Tool extraction | High | Low | High | Depends on tool vendors |
| First-line JSON | Medium | Low | Low | Easy |

---

## Recommendation

**Manifest file is the cleanest path.**

1. No source file changes
2. JSON is natively parseable
3. LLM queries one file, not N files
4. Can be generated from existing frontmatter
5. Sync via git hooks / CI

**But** - this is the approach I originally dismissed as "overcomplicating."

The user pushed back: "You are picking around the edges."

**Revisiting:** The manifest isn't overcomplicating. The inline comment format is the wrong abstraction. Manifest is the right abstraction for LLM consumption.

---

## Next Steps

1. Keep inline frontmatter for **human** readability
2. Generate `.fmm/index.json` manifest for **LLM** queryability
3. LLM workflow: query manifest → targeted file reads
4. Best of both worlds

---

## The Updated Thesis

```
Frontmatter in comments = invisible to LLMs (skipped)
Frontmatter in manifest = queryable by LLMs (used)

Generate both:
- Inline comments for humans
- Manifest JSON for LLMs
```

---

*Insight captured: 2026-01-28*
*This changes the fmm roadmap.*
