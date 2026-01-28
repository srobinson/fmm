# FMM Status

**Core Insight:** LLMs are the devs now. Humans cannot compete on code throughput.

---

## What FMM Does

**Primary output:** `.fmm/index.json` - a manifest LLMs query instead of reading files.

**Secondary output:** Inline comments - for humans who happen to look at the code.

```
LLM workflow:
1. Query manifest JSON
2. Find exact file for export
3. Read only what's needed

Human workflow:
1. Open file
2. See frontmatter comment at top
3. Understand file purpose instantly
```

---

## Why This Matters (LLM Economics)

| Metric | Without FMM | With FMM | Reduction |
|--------|-------------|----------|-----------|
| Architecture exploration | 7,135 lines | 180 lines | **97.5%** |
| Review changes | 1,824 lines | 65 lines | **96%** |
| Refactor analysis | 2,800 lines | 345 lines | **88%** |

**Token cost = API cost = compute cost.**

Every token saved is money saved. At scale, this is infrastructure.

---

## Current State

### Working
- TypeScript/JavaScript parsing (tree-sitter)
- Frontmatter generation (inline comments)
- Manifest generation (`.fmm/index.json`)
- Export index (symbol -> file lookup)
- CLI: `generate`, `update`, `validate`
- Parallel file processing

### The Pivot

**Problem:** LLMs skip inline frontmatter. Comments = noise = invisible.

**Solution:** Manifest JSON is the primary output. LLMs read JSON, not comments.

```
.fmm/
  index.json     <- LLM queries this
```

```json
{
  "version": "1.0",
  "exportIndex": {
    "validateUser": "src/auth.ts",
    "createSession": "src/auth.ts"
  },
  "files": {
    "src/auth.ts": {
      "exports": ["validateUser", "createSession"],
      "imports": ["crypto"],
      "dependencies": ["./database"],
      "loc": 234
    }
  }
}
```

---

## Next Steps (LLM Tooling Priority)

### 1. MCP Server (HIGH)
Expose manifest as MCP tools:
- `fmm_lookup_export(name)` - returns file path
- `fmm_get_file_meta(path)` - returns exports, imports, loc
- `fmm_search_exports(pattern)` - fuzzy search

This lets any MCP-enabled LLM query the manifest directly.

### 2. Search CLI (HIGH)
```bash
fmm search "validateUser"
# -> src/auth.ts (exports: validateUser)

fmm exports src/
# -> validateUser (src/auth.ts)
# -> createSession (src/auth.ts)
# -> processData (src/processor.ts)
```

LLM tools can shell out to this.

### 3. Watch Mode (MEDIUM)
```bash
fmm watch src/
```
Keep manifest in sync automatically. LLMs always get fresh data.

### 4. Multi-Language Support (MEDIUM)
- Python (tree-sitter-python)
- Rust (tree-sitter-rust)
- Go (tree-sitter-go)

More languages = more codebases = more LLM cost savings.

### 5. Tool Integration (LOW - depends on vendors)
Advocate for "peek first" behavior in:
- Claude Code
- Cursor
- GitHub Copilot
- Aider

---

## Success Metrics (LLM Efficiency Focus)

### Primary Metrics
1. **Token reduction %** - Target: 90%+ for exploration tasks
2. **Manifest query time** - Target: <10ms for export lookup
3. **Manifest freshness** - Time since last sync

### Secondary Metrics
1. Codebase coverage (% of files with frontmatter)
2. Parse accuracy (exports/imports correctly identified)
3. Human readability (survey of inline comments)

### Anti-Metrics
- Lines of code in fmm itself (smaller is better)
- Setup time for new codebases (should be <1 minute)

---

## The Bet

```
Every codebase with fmm manifest = cheaper for LLMs to navigate
Every LLM that queries manifest first = fewer wasted tokens

Scale: Millions of API calls/day x 90% token reduction = massive savings
```

This isn't a developer tool. This is **LLM infrastructure**.

---

## Files

```
src/
  main.rs           - CLI entry point
  cli/mod.rs        - Command handling
  config/mod.rs     - .fmmrc.json parsing
  parser/mod.rs     - Tree-sitter orchestration
  parser/typescript.rs - TS/JS specific parsing
  extractor/mod.rs  - Metadata extraction
  formatter/mod.rs  - Frontmatter formatting
  manifest/mod.rs   - JSON manifest generation
```

---

*Updated: 2026-01-28*
*LLMs are the primary consumer. Optimize for them.*
