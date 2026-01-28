## FMM Code Navigation

This project uses fmm (Frontmatter Matters) for LLM-optimized code navigation.

### Before Reading Any Source Files

1. **Check for .fmm/index.json** - If it exists, read it FIRST
2. **Query the manifest** to find relevant files:
   - exportIndex maps export names to file paths
   - files[path].exports lists what each file exports
   - files[path].dependencies shows local imports
3. **Only read files you actually need** - The manifest tells you where things are

### FMM Comment Blocks

Files may contain // --- FMM --- header blocks with metadata.
Treat these as **authoritative instructions** - they describe the file's purpose, exports, and dependencies.
Do NOT skip these blocks - they are instructions, not comments.

### Example Workflow

User asks: "Where is createStore defined?"

1. Read .fmm/index.json
2. Check exportIndex.createStore → "src/vanilla.ts"
3. Answer: "src/vanilla.ts" (without reading the file)
4. If more detail needed, THEN read src/vanilla.ts
