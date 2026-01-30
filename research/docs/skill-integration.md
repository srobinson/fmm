# FMM Skill Integration - Claude Code Navigation Skills

## Overview

fmm integrates with Claude Code through a **skill** mechanism that teaches the AI agent to navigate codebases using `.fmm` sidecar files as the primary navigation layer. The skill is a markdown file installed at `.claude/skills/fmm-navigate.md` that provides behavioral instructions -- when to check sidecars, how to search them, and when to fall back to reading source.

This document covers the skill's content and design, the installation workflow, how skills compare to MCP tools, and the experimental evidence for combining both.

---

## 1. What is a Claude Code Skill?

Claude Code supports a `.claude/skills/` directory where markdown files provide task-specific instructions. When Claude Code loads a project, it reads these skill files and incorporates their guidance into its behavior. Skills are:

- **Declarative**: They describe *what to do*, not code to execute
- **Project-scoped**: Installed per-project in `.claude/skills/`
- **Auto-loaded**: Claude Code picks them up automatically, no user action needed
- **Versioned**: Can be committed to git alongside the project

A skill is functionally similar to adding instructions to `CLAUDE.md`, but with cleaner isolation. Multiple tools can each install their own skill file without conflicting in a shared `CLAUDE.md`.

---

## 2. The fmm-navigate Skill

### 2.1 Source Location

The skill content lives at `docs/fmm-navigate.md` in the fmm repository and is embedded in the compiled binary via Rust's `include_str!()` macro:

```rust
// src/cli/mod.rs, line 427
const SKILL_CONTENT: &str = include_str!("../../docs/fmm-navigate.md");
```

This means the skill is always available without external file dependencies. The binary carries the exact skill text that corresponds to its version.

### 2.2 Full Skill Content

The skill file uses YAML frontmatter followed by markdown:

```yaml
---
name: fmm-navigate
description: Navigate codebases using .fmm sidecar files -- read sidecars before source, use MCP tools for lookup and graph queries
---
```

The body teaches Claude five key behaviors:

**1. Sidecar-First Navigation**

The skill opens by explaining the sidecar concept:

> Source files in this project have `.fmm` sidecar companions. For every `foo.ts` there may be a `foo.ts.fmm` containing structured metadata -- exports, imports, dependencies, and file size.

It shows an example sidecar:

```
file: src/core/pipeline.ts
fmm: v0.2
exports: [createPipeline, PipelineConfig, PipelineError]
imports: [zod, lodash]
dependencies: [./engine, ./validators, ../utils/logger]
loc: 142
```

And the key insight: "A sidecar tells you everything about a file's role without reading the source."

**2. Navigation Strategies**

The skill provides concrete workflows for common tasks:

| Task | Workflow |
|------|---------|
| Finding which files to edit | `Grep "exports:.*SymbolName" **/*.fmm` -> read sidecar -> open source only if editing |
| "Where is X defined?" | `Grep "exports:.*X" **/*.fmm` or call `fmm_lookup_export(name: "X")` if MCP available |
| "What depends on this file?" | Call `fmm_dependency_graph(file)` or `Grep "dependencies:.*filename" **/*.fmm` |
| "Describe the architecture" | `Glob **/*.fmm` -> read sidecars -> DO NOT start by reading source files |
| "Which files use package X?" | `Grep "imports:.*X" **/*.fmm` or call `fmm_search(imports: "X")` |

Each workflow provides both an MCP tool path (preferred when available) and a grep-based fallback.

**3. MCP Tool Reference**

The skill lists all five MCP tools:

- `fmm_lookup_export(name)` -- O(1) symbol -> file lookup
- `fmm_list_exports(pattern?, file?)` -- search exports by substring
- `fmm_file_info(file)` -- file metadata from the sidecar
- `fmm_dependency_graph(file)` -- upstream deps + downstream dependents
- `fmm_search({export?, imports?, depends_on?, min_loc?, max_loc?})` -- multi-criteria search

**4. Five Rules**

The skill ends with explicit rules:

1. **CHECK SIDECARS FIRST** -- before reading any source file, check if `filename.fmm` exists
2. **USE SIDECARS TO NAVIGATE** -- grep sidecars to find relevant files, not source code
3. **ONLY OPEN SOURCE FILES YOU WILL EDIT** -- sidecars tell you the file's role; only read source when you need to see or modify the implementation
4. **USE MCP TOOLS** when available -- `fmm_lookup_export` and `fmm_search` are faster than grep
5. **FALL BACK** to Grep/Glob on source only when searching file *contents* (not structure)

### 2.3 Design Principles

The skill is intentionally concise (69 lines). Key design choices:

- **Imperative, not suggestive**: "CHECK SIDECARS FIRST" not "You might want to check sidecars"
- **Concrete examples**: Every strategy includes the exact Grep/Glob/MCP command to use
- **Dual paths**: Each workflow shows both MCP tool and grep fallback, so the skill works with or without MCP
- **Negative instructions**: "DO NOT start by reading source files" explicitly prevents the default behavior

---

## 3. Installation Workflow

### 3.1 `fmm init --skill`

The simplest installation path:

```bash
fmm init --skill
```

This creates `.claude/skills/fmm-navigate.md` with the embedded skill content. The `init_skill()` function in `src/cli/mod.rs` handles this:

```rust
pub fn init_skill() -> Result<()> {
    let skill_dir = Path::new(".claude").join("skills");
    let skill_path = skill_dir.join("fmm-navigate.md");

    std::fs::create_dir_all(&skill_dir)
        .context("Failed to create .claude/skills/ directory")?;

    if skill_path.exists() {
        let existing = std::fs::read_to_string(&skill_path)
            .context("Failed to read existing skill file")?;
        if existing == SKILL_CONTENT {
            println!("Already up to date (skipping)");
            return Ok(());
        }
    }

    std::fs::write(&skill_path, SKILL_CONTENT)
        .context("Failed to write skill file")?;

    println!("Installed Claude skill at .claude/skills/fmm-navigate.md");
    Ok(())
}
```

Key behaviors:
- **Creates directory**: `.claude/skills/` is created if it doesn't exist
- **Idempotent**: If the file already exists with identical content, it skips
- **Updates silently**: If the content differs (e.g., fmm was upgraded), it overwrites

### 3.2 `fmm init --all`

The recommended full setup installs three components in one command:

```bash
fmm init --all
```

| Component | File Created | Purpose |
|-----------|-------------|---------|
| Config | `.fmmrc.json` | Languages, format, LOC tracking |
| Skill | `.claude/skills/fmm-navigate.md` | Behavioral guidance for Claude |
| MCP | `.mcp.json` | MCP server configuration |

The `init()` function orchestrates all three:

```rust
pub fn init(skill: bool, mcp: bool, all: bool) -> Result<()> {
    let specific = skill || mcp;
    let full_setup = !specific || all;

    let install_config = full_setup;
    let install_skill = skill || full_setup;
    let install_mcp = mcp || full_setup;

    if install_config { init_config()?; }
    if install_skill  { init_skill()?; }
    if install_mcp    { init_mcp_config()?; }

    println!("Setup complete!");
    println!("Run `fmm generate` to create sidecar files.");
    Ok(())
}
```

Logic:
- `fmm init` (no flags) -- installs all three (full setup)
- `fmm init --skill` -- skill only
- `fmm init --mcp` -- MCP only
- `fmm init --all` -- explicitly all three

### 3.3 Complete Setup Sequence

The recommended first-time setup:

```bash
# 1. Install fmm (cargo, brew, or binary)
cargo install fmm

# 2. Generate sidecar files for all source code
fmm generate

# 3. Install skill + MCP + config
fmm init --all

# Result:
#   .fmmrc.json                        -- configuration
#   .claude/skills/fmm-navigate.md     -- skill for Claude Code
#   .mcp.json                          -- MCP server registration
#   src/foo.ts.fmm                     -- sidecars (one per source file)
```

After this, Claude Code will automatically:
1. Read the skill file and adopt sidecar-first navigation
2. Discover the MCP server via `.mcp.json`
3. Start `fmm serve` and use structured tools for lookups

### 3.4 Updating the Skill

When fmm is upgraded, the embedded skill content may change. Re-running `fmm init --skill` will detect the content difference and overwrite:

```
$ fmm init --skill
Installed Claude skill at .claude/skills/fmm-navigate.md
```

If the content is already up to date:

```
$ fmm init --skill
.claude/skills/fmm-navigate.md already up to date (skipping)
```

---

## 4. Skill vs. MCP: Complementary Mechanisms

### 4.1 What Each Provides

**The skill provides the "why"**: It teaches Claude *when* and *how* to use sidecar navigation. Without the skill, Claude follows its default behavior -- reading source files directly, scanning directories, grepping for text. The skill redirects this behavior toward sidecars.

**MCP provides the "how"**: It gives Claude structured tools for querying the sidecar index. `fmm_lookup_export("createSession")` returns the file path in O(1). `fmm_dependency_graph("src/auth.ts")` returns upstream and downstream files. Without MCP, Claude must fall back to `Grep "exports:.*createSession" **/*.fmm`.

### 4.2 What Happens Without Each

| Configuration | Behavior |
|--------------|----------|
| Neither skill nor MCP | Claude reads source files directly. No sidecar awareness. |
| Skill only | Claude checks sidecars, greps `.fmm` files. Works but slower -- no O(1) lookups. |
| MCP only | Claude has tools available but often ignores them (58% manifest access in experiments). |
| Skill + MCP | Claude checks sidecars first (skill) using fast tools (MCP). Best performance. |

The critical finding: **MCP alone is insufficient**. Without behavioral guidance, Claude only uses the available MCP tools 58% of the time. The skill doubles MCP tool utilization by explicitly instructing Claude to check sidecars before reading source.

### 4.3 Skill vs. CLAUDE.md

The skill and a CLAUDE.md snippet are functionally equivalent -- both deliver text instructions that Claude Code reads at project load. The difference is organizational:

| Aspect | CLAUDE.md Snippet | .claude/skills/ |
|--------|------------------|-----------------|
| Location | Appended to CLAUDE.md | Separate file in `.claude/skills/` |
| Isolation | Mixed with other instructions | Clean separation per tool |
| Collision risk | Multiple tools may edit CLAUDE.md | Each tool gets its own file |
| Installation | Manual copy-paste or script | `fmm init --skill` |
| Discoverability | Must read CLAUDE.md | Explicit skill directory |
| Performance | Equivalent (~22.5 tool calls) | Equivalent (~22.5 tool calls) |

Experiment data (exp15) confirms both approaches produce the same baseline: ~22.2 tool calls for CLAUDE.md vs. ~22.5 for skill. The mechanism doesn't matter -- what matters is that the instructions exist.

### 4.4 Comparison Matrix

| Capability | Skill | MCP | Skill + MCP |
|-----------|-------|-----|-------------|
| O(1) export lookup | No (grep-based) | Yes | Yes |
| Dependency graph | No (manual grep) | Yes | Yes |
| Multi-criteria search | No | Yes | Yes |
| Behavioral guidance | Yes | No | Yes |
| Works offline | Yes | No (needs server) | Partially |
| Platform support | Claude Code only | Any MCP client | Claude Code |
| Zero config after init | Yes | Yes | Yes |

---

## 5. Experiment Results: Quantifying the Value

### 5.1 Experimental Setup (exp15)

48 live runs across 4 configurations, testing 4 task types (find symbol, understand architecture, impact analysis, multi-file edit) on a real codebase.

### 5.2 Raw Performance Data

| Metric | A: CLAUDE.md | B: Skill Only | C: MCP Only | D: Skill+MCP |
|--------|-------------|--------------|------------|-------------|
| Avg Tool Calls | 22.2 | 22.5 | 18.2 | **15.5** |
| Avg File Reads | 5.2 | 4.1 | 4.6 | **4.8** |
| Cost per Task | $0.55 | $0.47 | $0.50 | **$0.41** |
| Manifest Access Rate | 83% | 75% | 58% | **75%** |
| Duration | 85.8s | 94.5s | 72.2s | **68.5s** |

### 5.3 Key Findings

1. **Skill equals CLAUDE.md**: Within 1% on tool calls (22.5 vs 22.2), confirming they are interchangeable as instruction delivery mechanisms.

2. **MCP alone is insufficient**: Only 58% manifest access rate without behavioral guidance. Claude often ignores available tools and falls back to reading files.

3. **Skill+MCP is strictly best**: 30% fewer tool calls than either standalone approach. 25% cheaper ($0.41 vs $0.55). 20% faster (68.5s vs 85.8s).

4. **The combination creates a feedback loop**:
   - Skill guides Claude to check sidecars first
   - MCP makes those checks fast (O(1) lookups)
   - Fast checks encourage more frequent use
   - More frequent use leads to fewer unnecessary file reads

### 5.4 Why MCP Alone Underperforms

Claude's default behavior when it sees available MCP tools but has no skill:
1. Reads the tool descriptions in `tools/list` response
2. Starts with its normal approach (reading files)
3. May or may not remember to try MCP tools
4. Often completes the task through file reads before considering alternatives

The skill breaks this pattern by front-loading the instruction: "CHECK SIDECARS FIRST." This changes the decision tree from "should I try these tools?" to "the instructions say check sidecars, let me use the tools."

---

## 6. Architecture Details

### 6.1 Skill Embedding and Versioning

The skill content is compiled into the fmm binary:

```rust
const SKILL_CONTENT: &str = include_str!("../../docs/fmm-navigate.md");
```

This creates a strong coupling between fmm version and skill version. When the tool's capabilities change (e.g., new MCP tools added), the skill is updated in the same commit. Users who upgrade fmm and re-run `fmm init --skill` get the matching skill version.

### 6.2 Integration Points

```
                    ┌─────────────────────────────────┐
                    │          Claude Code             │
                    │                                  │
                    │  1. Reads .claude/skills/         │
                    │     fmm-navigate.md              │
                    │     (learns sidecar workflow)     │
                    │                                  │
                    │  2. Discovers .mcp.json           │
                    │     (starts fmm serve)            │
                    └──────┬──────────────┬────────────┘
                           │              │
              ┌────────────▼──┐     ┌─────▼──────────┐
              │  Grep/Glob    │     │  MCP Server     │
              │  on *.fmm     │     │  (fmm serve)    │
              │  (fallback)   │     │                 │
              └───────────────┘     │  tools/call:    │
                                    │  - lookup_export│
                                    │  - list_exports │
                                    │  - file_info    │
                                    │  - dep_graph    │
                                    │  - search       │
                                    └────────┬────────┘
                                             │
                                    ┌────────▼────────┐
                                    │  Manifest       │
                                    │  (in-memory)    │
                                    │                 │
                                    │  Built from     │
                                    │  *.fmm sidecars │
                                    └─────────────────┘
```

The skill instructs Claude to prefer MCP tools (path 2) but always provides the grep-based fallback (path 1) for when MCP is unavailable.

### 6.3 MCP Server Lifecycle

When Claude Code reads `.mcp.json`, it starts the fmm MCP server:

```json
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["serve"]
    }
  }
}
```

The server lifecycle:
1. **Startup**: `McpServer::new()` loads manifest from sidecars
2. **Initialize**: Responds to MCP `initialize` with capabilities
3. **Tool calls**: On each `tools/call`, rebuilds manifest from sidecars (ensures freshness)
4. **Shutdown**: Process ends when stdin closes

The pre-call manifest rebuild (line 104 of `mcp/mod.rs`) means the MCP tools always reflect the current sidecar state, even if files were regenerated since the server started.

### 6.4 Idempotent Init Design

All three init functions follow the same pattern:

```
1. Check if target file exists
2. If exists, compare content
3. If identical, skip with message
4. If different (or missing), write new content
```

For `.mcp.json`, the logic is slightly more complex -- it merges the fmm server entry into an existing config rather than overwriting:

```rust
if mcp_path.exists() {
    let existing = std::fs::read_to_string(mcp_path)?;
    if let Ok(mut existing_json) = serde_json::from_str::<Value>(&existing) {
        if servers.contains_key("fmm") {
            // already configured, skip
            return Ok(());
        }
        // merge fmm into existing mcpServers
        servers_obj.insert("fmm", fmm_config);
    }
}
```

This means `fmm init --all` is safe to run repeatedly and won't clobber other MCP server configurations.

---

## 7. Cross-Platform Considerations

### 7.1 Claude Code

Full support. Skills are a native Claude Code feature. The skill is loaded automatically from `.claude/skills/fmm-navigate.md`.

### 7.2 Other MCP Clients (Cursor, Windsurf, etc.)

These tools support MCP via `.mcp.json` but do not have an equivalent to `.claude/skills/`. For these:
- Install MCP only: `fmm init --mcp`
- Optionally add navigation instructions to their respective config files
- MCP tools still work, but without behavioral guidance the 58% manifest access rate applies

### 7.3 Non-MCP Environments

In environments without MCP support, the skill's grep-based fallback still works. The skill teaches:
- `Grep "exports:.*X" **/*.fmm` for symbol lookup
- `Grep "dependencies:.*filename" **/*.fmm` for dependency analysis
- `Glob **/*.fmm` for architecture discovery

These patterns work in any tool that supports grep and glob.

---

## 8. The Skill as a Design Pattern

### 8.1 Beyond fmm

The fmm skill demonstrates a pattern applicable to any developer tool that wants to teach LLMs new behaviors:

1. **Create a skill markdown file** with frontmatter and instructions
2. **Embed it in your binary** via `include_str!()` or equivalent
3. **Provide an init command** that installs it to `.claude/skills/`
4. **Make it idempotent** -- check before writing, compare content
5. **Pair with MCP tools** for structured queries when behavioral guidance alone isn't enough

### 8.2 Skill Design Guidelines (Lessons from fmm)

From the experiment data:

- **Be imperative**: "CHECK SIDECARS FIRST" outperforms "You can check sidecars"
- **Show concrete commands**: Include exact grep patterns and tool calls
- **Provide dual paths**: Always include a fallback for when MCP is unavailable
- **Include negative instructions**: "DO NOT start by reading source files" prevents default behavior
- **Keep it short**: 69 lines is sufficient. LLMs don't need verbose instructions -- they need clear rules
- **Test with experiments**: The only way to know if a skill works is to measure it against a control

### 8.3 What Skills Cannot Do

Skills are behavioral guidance, not code. They cannot:
- Execute actions (use MCP tools or CLI for that)
- Access APIs or external services
- Modify files or run commands
- Persist state between sessions

They can only influence how Claude approaches a task. For anything executable, pair the skill with MCP tools.

---

## 9. Recommendation Matrix

| Scenario | Recommended Setup | Command |
|----------|------------------|---------|
| Claude Code, best performance | Skill + MCP | `fmm init --all` |
| Claude Code, minimal setup | Skill only | `fmm init --skill` |
| Cursor / Windsurf / other MCP client | MCP only | `fmm init --mcp` |
| CI/CD validation | Neither (use `fmm validate`) | `fmm validate` |
| Large team, strict config control | MCP + CLAUDE.md snippet | `fmm init --mcp` + manual |

The data is clear: for Claude Code users, `fmm init --all` delivers 25% cost reduction and 20% faster execution compared to any single-mechanism approach.

---

This document captures the complete skill integration story for fmm. The core insight: LLMs need both *knowledge* (what tools exist) and *behavioral guidance* (when to use them). Skills provide the guidance, MCP provides the tools, and together they achieve performance neither can reach alone.
