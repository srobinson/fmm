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
- `fmm_lookup_export(name)` - Find file by export name, returns path + metadata
- `fmm_list_exports(pattern?, file?)` - Search exports by pattern, or list a file's exports
- `fmm_file_info(file)` - Get structured metadata for a specific file
- `fmm_dependency_graph(file)` - Get upstream/downstream dependencies for a file
- `fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})` - Multi-criteria search
- `fmm_get_manifest()` - Get full project structure
```
