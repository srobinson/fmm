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

### Option 2: Manifest File ← **THE ANSWER**

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
- **Sync:** Git hooks / CI / watch mode - this is solved infrastructure

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

| Approach | LLM Visibility | LLM Cost Impact | Maintenance | Adoption Path |
|----------|----------------|-----------------|-------------|---------------|
| Code export | High | Medium savings | Per-file overhead | Hard (bundler issues) |
| **Manifest file** | **High** | **94%+ reduction** | **Automated** | **Clear winner** |
| Tool extraction | High | Medium savings | Vendor dependent | Blocked by tool vendors |
| First-line JSON | Medium | Low savings | Per-file overhead | LLMs still skip comments |

---

## Recommendation

**Manifest file is the only viable path.**

1. No source file changes required
2. JSON is natively parseable by LLMs
3. LLM queries ONE file to understand ENTIRE codebase
4. Generated automatically via static analysis
5. Sync via git hooks / CI / watch mode

**The insight:** We were optimizing for the wrong user.

Inline comments are human-readable. But humans aren't reading codebases at scale anymore - LLMs are.

**LLMs are the devs now.** Build the infrastructure they need.

---

## Next Steps

1. Generate `.fmm/index.json` manifest as **primary output**
2. LLM workflow: query manifest → targeted file reads
3. Inline frontmatter is optional (legacy/human tooling only)

---

## The Updated Thesis

```
LLMs are the devs now. Humans cannot compete.

Frontmatter in comments = invisible to LLMs (skipped)
Frontmatter in manifest = queryable by LLMs (used)

The manifest is the product. Inline is the byproduct.
```

---

## The Economic Reality

Every token an LLM reads costs money. Manifest JSON:
- One query to understand the entire codebase structure
- Targeted reads only when needed
- 94%+ token reduction = 94%+ cost reduction

**LLMs are the primary consumers of code metadata. Build for them.**

---

*Insight captured: 2026-01-28*
*This changes the fmm roadmap.*
