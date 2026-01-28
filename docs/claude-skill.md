# fmm Claude Skill

Add to your `.claude/skills/` directory as `fmm.md`:

```markdown
---
name: fmm
description: Navigate code using fmm manifest
---

# /fmm - Code Navigation

When exploring this codebase, use fmm for efficient navigation:

## Before Reading Files

1. Check if `.fmm/index.json` exists
2. Query the manifest to find relevant files:
   - By export: Look up `exportIndex` for symbol → file mapping
   - By imports: Filter files by their imports array
   - By size: Check `loc` field to prioritize

## Using Frontmatter

Files with `// --- FMM ---` headers contain metadata.
Read first 15 lines to get: exports, imports, dependencies, LOC.

## CLI Commands

```bash
fmm search --export validateUser    # Find file by export
fmm search --imports crypto         # Files importing crypto
fmm search --loc ">500"             # Large files
```

## MCP Tools (if available)

Use these tools directly:
- `fmm_find_export(name)` → file path
- `fmm_list_exports(file)` → exports array
- `fmm_search({export?, imports?, min_loc?, max_loc?})` → matching files
- `fmm_get_manifest()` → full project metadata

## Workflow

1. Get manifest structure first
2. Identify relevant files by exports/imports
3. Read only the files you need
4. Use frontmatter headers for quick triage
```
