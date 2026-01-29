# exp15: Skill vs CLAUDE.md vs MCP — Findings

**Date:** 2026-01-29
**Status:** Protocol designed, theoretical analysis complete, live runs pending
**Predecessor:** exp13 (88-97% token reduction with manifest + instructions)

---

## Executive Summary

Three instruction delivery mechanisms for fmm manifest integration were analyzed:
1. **CLAUDE.md** — project-level instructions (exp13 baseline)
2. **Claude Skill** — installable instruction package
3. **MCP Server** — tool-based integration

**Recommendation:** Ship `fmm init --all` as default. Install both Skill + MCP.
The skill provides the "why" (when/how to use the manifest), MCP provides the "how" (structured queries). Neither alone is optimal.

---

## Mechanism Analysis

### Condition A: CLAUDE.md Only (exp13 baseline)

**How it works:** User adds fmm navigation instructions to their project's CLAUDE.md. Claude reads this at session start and follows the instructions to check the manifest first.

**Strengths:**
- Proven: 88-97% token reduction in exp13
- Simple: Just text in a file Claude already reads
- Universal: Works with any Claude-based tool that reads CLAUDE.md

**Weaknesses:**
- **User friction:** Users are protective of their CLAUDE.md; adding tool-specific instructions feels invasive
- **Collision risk:** Multiple tools generating CLAUDE.md content creates merge conflicts
- **No structure:** Instructions are free-text; Claude interprets them variably
- **Manual maintenance:** If fmm tools change, CLAUDE.md needs manual update

**Expected performance:** Baseline (91% tool call reduction per exp13).

---

### Condition B: Skill Only

**How it works:** `fmm init --skill` installs `.claude/skills/fmm-navigate.md`. Claude Code loads skills automatically. The skill teaches Claude to check the manifest and use CLI commands.

**Strengths:**
- **Clean isolation:** Skills are separate from CLAUDE.md — no collision
- **Auto-loaded:** Claude Code discovers skills automatically
- **Versioned with fmm:** `include_str!()` means skill content ships with the binary
- **Idempotent:** `fmm init --skill` is safe to re-run

**Weaknesses:**
- **Claude Code specific:** Other tools (Cursor, Aider) don't have a skills mechanism
- **No structured queries:** Without MCP, Claude must Read the manifest file and parse JSON manually
- **CLI fallback overhead:** `fmm search` via Bash tool is slower than MCP tool call

**Expected performance:** Within 5-10% of CLAUDE.md baseline. Skills and CLAUDE.md are functionally equivalent — both inject instructions at session start. The skill is slightly more structured (has frontmatter metadata, clear sections) which may help Claude follow instructions more consistently.

**Prediction: Skill ≈ CLAUDE.md ± 10%**

---

### Condition C: MCP Only (no instructions)

**How it works:** `fmm init --mcp` adds fmm server to `.mcp.json`. Claude sees `fmm_*` tools in its tool list but receives no instructions about when to use them.

**Strengths:**
- **Zero config:** Just run `fmm init --mcp` — no instructions to write
- **Structured queries:** `fmm_lookup_export`, `fmm_dependency_graph` return precise JSON
- **Hot-reload:** Manifest changes are picked up automatically
- **Universal:** Any MCP client (Claude Code, Cursor, etc.) can use it

**Weaknesses:**
- **Discovery problem:** Claude may not realize fmm tools exist or when to use them
- **No behavioral guidance:** Without instructions, Claude defaults to Glob/Grep/Read
- **Tool description is the only hint:** Claude must infer workflow from tool descriptions alone

**Expected performance:** Significantly worse than A/B for exploration tasks. Tool availability alone doesn't change behavior — LLMs need to be told *when* to use tools, not just *that* they exist. For targeted lookups where Claude happens to search for an export, it might discover `fmm_lookup_export` from the tool description.

**Prediction: MCP only ≈ 30-50% of baseline effectiveness**

The tool descriptions say things like "Find which file exports a given symbol" — Claude may use this when it would otherwise grep. But for architecture exploration or impact analysis, Claude won't spontaneously think "I should check the fmm manifest first."

---

### Condition D: Skill + MCP (Full Integration)

**How it works:** Both skill and MCP server are active. The skill tells Claude about MCP tools and when to use them. MCP provides efficient structured queries.

**Strengths:**
- **Best of both worlds:** Instructions (skill) + capabilities (MCP)
- **Structured dependency queries:** `fmm_dependency_graph` is strictly better than manually parsing manifest JSON
- **Pattern matching:** `fmm_list_exports(pattern)` is more efficient than reading full manifest
- **Hot-reload:** Manifest stays current during long sessions

**Weaknesses:**
- **Two things to install:** More setup steps (mitigated by `fmm init --all`)
- **Redundancy:** Skill describes CLI commands that are less useful when MCP is available

**Expected performance:** Best overall. The skill ensures Claude knows to use fmm tools for every task. MCP makes those queries efficient and structured. For dependency graph queries specifically, D should outperform B significantly because `fmm_dependency_graph` computes upstream/downstream in one call, vs. B where Claude must read the manifest and manually trace dependencies.

**Prediction: Skill + MCP ≈ 95-100% of baseline + faster execution**

---

## Theoretical Comparison Matrix

| Metric | A (CLAUDE.md) | B (Skill) | C (MCP only) | D (Skill+MCP) |
|--------|--------------|-----------|--------------|---------------|
| Manifest discovery | Instructed | Instructed | Spontaneous (unlikely) | Instructed |
| Query efficiency | Read + parse JSON | Read + parse JSON | Structured tools | Structured tools |
| Dependency analysis | Manual trace | Manual trace | One tool call | One tool call |
| Export lookup | Manual search | Manual search | One tool call | One tool call |
| Setup friction | Medium (edit CLAUDE.md) | Low (one command) | Low (one command) | Low (one command) |
| Tool portability | Claude-specific | Claude Code specific | Any MCP client | Claude Code + MCP |
| Token reduction (est.) | 88-97% | 85-97% | 40-60% | 90-98% |
| User adoption friction | High | Low | Low | Low |

---

## Recommendation: Default Distribution Strategy

### Ship: `fmm init --all` (Skill + MCP)

**Rationale:**

1. **Skill alone is good but not optimal.** It teaches Claude to use the manifest but forces it to parse JSON manually. For simple lookups this is fine, but dependency graph traversal is clumsy without MCP.

2. **MCP alone is insufficient.** Without behavioral instructions, Claude doesn't know to check fmm tools first. The tools sit unused for exploration tasks.

3. **Skill + MCP is the clear winner.** The skill provides behavioral guidance ("check the manifest first"), MCP provides efficient execution ("use `fmm_dependency_graph` instead of parsing JSON").

4. **`fmm init --all` is one command.** No additional friction over installing just one component.

### For non-Claude tools (Cursor, Aider):

- **MCP-capable tools:** `fmm init --mcp` + equivalent instructions mechanism
- **Non-MCP tools:** CLAUDE.md-style instructions (tool-specific config file)
- See ALP-376 for per-tool integration research

---

## Open Questions (for live runs)

1. **Does Claude discover MCP tools from descriptions alone?** The tool descriptions are fairly self-explanatory. It's possible that for certain tasks (like "find where X is defined"), Claude would try `fmm_lookup_export` even without instructions. Live runs needed to test this.

2. **Is there a difference between CLAUDE.md and Skill effectiveness?** Theoretically they're equivalent (both inject text at session start). But skills have structured frontmatter which might help Claude parse the instructions differently. Live runs needed.

3. **What's the marginal value of MCP over Skill-only?** For simple export lookups, Skill-only (read manifest JSON) might be fast enough. MCP's value shows on dependency graph queries. How often do users ask dependency questions? Live runs needed.

4. **Does hot-reload matter in practice?** During a session, if the user edits code and re-runs `fmm generate`, the MCP server picks up changes. Without MCP, Claude reads a stale manifest. How often does this matter? Probably only in long sessions with active development.

---

## Next Steps

1. **Run live experiments** using the protocol in `PROTOCOL.md`
2. **3 runs per condition** on the target codebase
3. **Grade accuracy** manually for each run
4. **Update this document** with empirical results

---

*Analysis: 2026-01-29*
*Collaborators: Stuart Robinson, Claude Opus 4.5*
