# CLAUDE.md Snippet for fmm

Add this to your project's CLAUDE.md to enable LLM-optimized code navigation:

```markdown
## Code Navigation

This project uses fmm for LLM-optimized code navigation.

### Manifest Location
- `.fmm/index.json` contains metadata for all source files
- Query this before reading files to understand project structure

### Frontmatter Headers
Files contain `// --- FMM ---` headers with exports, imports, and LOC.
Read just the first 15 lines to understand what a file does.

### Quick Commands
```bash
fmm search --export <name>    # Find file by export
fmm search --imports <module> # Find files importing module
fmm search --loc ">500"       # Find large files
```

### MCP Integration
If fmm MCP server is available, use these tools:
- `fmm_find_export(name)` - Find file by export
- `fmm_search(query)` - Search manifest
- `fmm_get_manifest()` - Get full project structure
```
