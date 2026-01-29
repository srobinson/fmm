# LLM Tool Integration Matrix for fmm

**Date:** 2026-01-29
**Purpose:** Document how Cursor, Aider, Windsurf, and Continue.dev handle codebase indexing, and identify where fmm can integrate.

---

## Tool Comparison Matrix

| Feature | Cursor | Aider | Windsurf | Continue.dev |
|---------|--------|-------|----------|-------------|
| **Indexing Method** | Embeddings (vector DB) | Tree-sitter AST + PageRank | RAG + M-Query | Embeddings + AST |
| **Index Storage** | Turbopuffer (cloud) | In-memory per session | Local + cloud | Local vector DB |
| **Sync Mechanism** | Merkle tree hashing | Re-computed per session | Real-time indexing engine | Workspace indexing |
| **MCP Support** | Yes (native) | No (community only) | Yes (native, Cascade client) | Yes (native, full spec) |
| **Project Instructions** | `.cursor/rules/*.mdc` | `--read` flag, conventions | `@-mentions`, guidelines | `.continue/rules/` |
| **CLAUDE.md Equivalent** | `.cursor/rules/` (always-on rules) | `.aider/conventions.md` | Custom guidelines | `.continue/rules/` |
| **Skill-like Mechanism** | `.cursor/rules/*.mdc` (agent-requested) | None | None | Hub assistants |
| **fmm Integration Path** | MCP + .cursor/rules | Repo map augmentation | MCP + guidelines | MCP + context provider |
| **Integration Effort** | Low | Medium-High | Low | Low |
| **Priority** | 1 (highest) | 3 | 2 | 2 |

---

## Cursor

### How It Indexes

Cursor uses a sophisticated embedding-based RAG pipeline:

1. **Chunking:** Source files are split into semantic chunks (functions, classes)
2. **Embedding:** Chunks are sent to Cursor's servers, embedded using AI models (likely Voyage AI or custom), content immediately discarded
3. **Storage:** Embeddings stored in Turbopuffer (vector DB) indexed by content hash
4. **Sync:** Merkle tree of file hashes synced every few minutes; only changed files re-embedded
5. **Query:** User queries are embedded, nearest-neighbor search in Turbopuffer retrieves relevant chunks

Privacy: Filenames obfuscated, code chunks encrypted in transit, embeddings are one-way. `.gitignore` and `.cursorignore` respected.

### Where fmm Fits

**Problem fmm solves:** Cursor's embedding search is semantic (similarity-based) but can miss structural relationships. fmm provides deterministic, exact metadata: this file exports X, imports Y, depends on Z.

**Integration points:**

1. **MCP Server (primary):** Cursor has native MCP support. Configure fmm as MCP server:
   ```json
   // .cursor/mcp.json or Cursor Settings → MCP
   {
     "mcpServers": {
       "fmm": { "command": "fmm", "args": ["serve"] }
     }
   }
   ```
   Cursor's Agent auto-discovers MCP tools and uses them when relevant. `fmm_lookup_export`, `fmm_dependency_graph` complement Cursor's semantic search with exact structural queries.

2. **Project Rules:** `.cursor/rules/fmm.mdc` with instructions to check manifest first:
   ```markdown
   ---
   description: Use fmm manifest for codebase navigation
   alwaysApply: true
   ---
   Check .fmm/index.json before reading source files.
   Use fmm_lookup_export(name) for symbol lookups.
   Use fmm_dependency_graph(file) for impact analysis.
   ```

3. **Pre-indexing augmentation:** fmm manifest could supplement Cursor's embeddings by providing exact export-to-file mappings. This is a structural overlay on top of semantic search.

**Effort:** Low. MCP is native, rules are file-based.

---

## Aider

### How It Indexes

Aider builds a "repo map" using tree-sitter:

1. **AST Parsing:** Tree-sitter parses every file into an Abstract Syntax Tree
2. **Tag Extraction:** Extracts definitions (`def`) and references (`ref`) for functions, classes, types
3. **Graph Construction:** Files are nodes, edges connect files with def/ref relationships
4. **PageRank:** Ranks identifiers by importance (most-referenced = most important)
5. **Token Budget:** Selects the most important symbols that fit within `--map-tokens` budget (default 1K tokens)
6. **Map Format:** Outputs key lines (signatures, definitions) from most important symbols

The repo map is regenerated per session (not persisted). It shows function signatures but not full implementations.

### Where fmm Fits

**Problem fmm solves:** Aider's repo map is computed from scratch each session and focuses on "most referenced" symbols. fmm provides pre-computed, persistent metadata with explicit export/import relationships.

**Integration points:**

1. **Repo map replacement/augmentation:** fmm manifest provides the same information as Aider's repo map (exports, imports, dependencies) but pre-computed. The `exportIndex` is essentially a pre-ranked symbol table.

   Challenge: Aider's repo map format is proprietary (text-based, showing code lines). fmm would need to output in Aider's expected format, or Aider would need to accept fmm's JSON.

2. **`--read` flag:** Aider supports `--read .fmm/index.json` to include the manifest as read-only context. This is the simplest integration — just tell Aider to read the manifest.

3. **MCP (community):** Aider doesn't have native MCP support. Community tools like `mcpm-aider` exist but are experimental. Not a reliable integration path yet.

4. **Convention file:** `.aider/conventions.md` could include fmm navigation instructions, similar to CLAUDE.md approach.

**Effort:** Medium-High. Best path is `--read` flag for now. Native integration would require PR to Aider's repo map system.

**Opportunity:** The RepoMapper project (standalone Aider repo map) has an MCP server. This validates the concept of exposing code structure via MCP. fmm could be positioned as a more comprehensive alternative.

---

## Windsurf

### How It Indexes

Windsurf (Cascade) uses a proprietary indexing engine:

1. **Full Codebase Indexing:** Indexes all files, not just open ones
2. **RAG + M-Query:** Uses LLM-powered retrieval-augmented generation with proprietary "M-Query" techniques
3. **Tiered Access:** Pro/Teams/Enterprise get expanded indexing limits and remote repo indexing
4. **Context Pinning:** Users can pin specific code elements for AI priority

Less publicly documented than Cursor's approach, but functionally similar (embedding-based semantic search over code).

### Where fmm Fits

**Problem fmm solves:** Same as Cursor — supplements semantic search with exact structural metadata.

**Integration points:**

1. **MCP Server (primary):** Windsurf has native MCP support (Cascade is an MCP client). Configuration similar to Cursor:
   ```json
   {
     "mcpServers": {
       "fmm": { "command": "fmm", "args": ["serve"] }
     }
   }
   ```
   Windsurf offers one-click MCP server setup in settings, plus curated MCP server marketplace.

2. **Custom Guidelines:** Windsurf supports project-level guidelines (similar to Cursor rules). fmm navigation instructions can be added here.

3. **Context Providers:** Windsurf's `@-mention` system could reference `.fmm/index.json` directly.

**Effort:** Low. MCP is native, similar setup to Cursor.

---

## Continue.dev

### How It Indexes

Continue.dev uses a hybrid approach:

1. **Embeddings:** Vector-based codebase indexing for semantic search
2. **AST Parsing:** Inspired by Aider's repo map for structural understanding
3. **Context Providers:** Extensible system for adding custom context sources
4. **MCP Integration:** Full MCP spec support (resources, prompts, tools, sampling)

Continue is open-source and highly extensible.

### Where fmm Fits

**Problem fmm solves:** Continue already has good indexing, but fmm provides pre-computed, deterministic metadata that's faster than re-indexing.

**Integration points:**

1. **MCP Server (primary):** Continue has the most complete MCP implementation:
   ```json
   // .continue/config.json or .continue/mcpServers/fmm.json
   {
     "mcpServers": {
       "fmm": { "command": "fmm", "args": ["serve"] }
     }
   }
   ```
   Continue supports SSE, Streamable HTTP, and stdio transports. Full tool/resource/prompt support.

2. **Context Provider:** Continue's extensible context provider system could natively integrate fmm:
   ```json
   {
     "contextProviders": [{
       "name": "fmm",
       "params": { "manifestPath": ".fmm/index.json" }
     }]
   }
   ```
   This would require a PR to Continue.dev to add fmm as a built-in context provider.

3. **Hub Assistant:** Continue Hub allows creating custom assistants with pre-configured MCP servers and rules. An "fmm-powered" assistant template could be published.

4. **Rules:** `.continue/rules/` supports project-level instructions, equivalent to CLAUDE.md.

**Effort:** Low for MCP. Medium for native context provider (requires PR).

---

## Priority Ranking

### Tier 1: Immediate (ship with v1.0)

1. **Claude Code** — Already implemented (MCP + Skill). Primary user base.
2. **Cursor** — Largest market share among AI IDEs. Native MCP support. Low effort.

### Tier 2: Near-term

3. **Windsurf** — Growing fast, native MCP. Similar setup to Cursor.
4. **Continue.dev** — Open source, excellent MCP support. Opportunity for native integration via PR.

### Tier 3: Future

5. **Aider** — No native MCP. Best path is `--read` flag or convention file. Wait for native MCP support.

---

## Recommended `fmm init` Output Per Tool

| Tool | Command | What It Installs |
|------|---------|-----------------|
| Claude Code | `fmm init --all` | `.claude/skills/fmm-navigate.md` + `.mcp.json` |
| Cursor | `fmm init --cursor` | `.cursor/rules/fmm.mdc` + `.cursor/mcp.json` |
| Windsurf | `fmm init --windsurf` | Windsurf MCP config + guidelines |
| Continue | `fmm init --continue` | `.continue/mcpServers/fmm.json` + rules |
| Aider | `fmm init --aider` | `.aider/conventions.md` with fmm instructions |

---

## Key Insight: MCP Is the Universal Integration Layer

All major AI coding tools except Aider support MCP natively. This means:

1. **fmm's MCP server is the killer feature** — one implementation, multiple tools
2. **Per-tool config files are the thin layer** — just tell each tool about the MCP server
3. **Instructions/rules are the behavioral layer** — each tool has its own mechanism

The architecture is:
```
fmm generate → .fmm/index.json
fmm serve → MCP server (stdio)
fmm init --<tool> → tool-specific config pointing to MCP server
```

This is exactly what we built in ALP-373 (MCP server) and ALP-374 (skill + init).

---

## Outreach Opportunities

1. **Cursor Marketplace:** Submit fmm as a curated MCP server
2. **Continue Hub:** Publish fmm-powered assistant template
3. **Aider PR:** Propose fmm manifest as alternative/supplement to repo map
4. **Windsurf MCP Directory:** List fmm MCP server for one-click setup
5. **Blog Post:** "How fmm gives your AI IDE 97% better codebase navigation"

---

*Research conducted: 2026-01-29*
*Sources: Cursor docs, Aider docs, Windsurf docs, Continue.dev docs, community forums*
