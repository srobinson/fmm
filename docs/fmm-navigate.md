---
name: fmm-navigate
description: Navigate codebases efficiently using fmm manifests — check metadata before reading files
---

# fmm — Codebase Navigation

This project uses **fmm** (Frontmatter Matters) to provide structured metadata about every source file. Use this metadata to navigate the codebase efficiently without reading files unnecessarily.

## Before Reading Any Source File

1. **Check `.fmm/index.json`** — If it exists, read it FIRST. It contains:
   - `exportIndex`: Maps every export name to its file path (O(1) lookup)
   - `files[path].exports`: What each file exports
   - `files[path].imports`: External packages each file uses
   - `files[path].dependencies`: Local files each file depends on
   - `files[path].loc`: Lines of code (use to prioritize what to read)

2. **Use the export index for targeted lookups:**
   - Need `validateUser`? Check `exportIndex.validateUser` → get the file path directly
   - Never grep the codebase for a function name if the manifest has it

3. **Use dependencies for impact analysis:**
   - Changing `src/auth.ts`? Check which files list it in their `dependencies`
   - This tells you the blast radius without reading every file

## MCP Tools (when available)

If the fmm MCP server is configured, use these tools directly:

- **`fmm_lookup_export(name)`** — Find which file exports a symbol. Returns file path + full metadata
- **`fmm_list_exports(pattern?, file?)`** — Search exports by substring pattern, or list a file's exports
- **`fmm_file_info(file)`** — Get structured metadata for a specific file
- **`fmm_dependency_graph(file)`** — Get upstream (dependencies) and downstream (dependents) for a file
- **`fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})`** — Multi-criteria search
- **`fmm_get_manifest()`** — Get full project structure

## CLI Fallback (when MCP is not available)

```bash
fmm search --export validateUser    # Find file by export name
fmm search --imports crypto         # Find files importing a module
fmm search --depends-on ./auth      # Find files depending on a path
fmm search --loc ">500"             # Find large files
fmm search --json                   # Machine-readable output
```

## Workflow

1. **Start with the manifest** — Read `.fmm/index.json` or call `fmm_get_manifest()`
2. **Identify relevant files** by exports, imports, or dependencies
3. **Read only the files you need** — The manifest tells you exactly where things are
4. **For impact analysis** — Use `fmm_dependency_graph(file)` to find upstream/downstream files
5. **Fall back to exploration** only if the manifest doesn't cover what you need

## Inline Frontmatter Headers

Source files may contain `// --- FMM ---` comment blocks at the top. These are human-readable summaries of the same metadata. Read just the first 15 lines to quickly understand what a file does without reading the full source.
