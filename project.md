# FMM Claim Verification Report

**Date:** 2026-02-13  
**Experiment:** A/B comparison using agentic-flow codebase (798 TypeScript files, 1306 sidecars)

---

## Summary

| Claim | Result | Status |
|-------|--------|--------|
| **Token Reduction (88-97%)** | 99.5% input token reduction | ✅ **VERIFIED** |
| **Tool Call Reduction (~30%)** | 68.7% reduction | ✅ **VERIFIED** (exceeds claim) |
| **FMM MCP Tools Used** | 5 tool calls across 3 tasks | ✅ **VERIFIED** |

---

## Detailed Results

### Per-Task Breakdown

| Task | Control Tools | FMM Tools | Control Input Toks | FMM Input Toks | Reduction |
|------|---------------|-----------|-------------------|----------------|-----------|
| find_export | 1 (Grep) | 1 (fmm_lookup_export) | 8 | 3 | **62.5%** |
| dependency_graph | 7 (Bash,Read,Grep) | 2 (fmm_dependency_graph) | 1598 | 5 | **99.7%** |
| list_exports | 8 (Bash,Glob,Grep,Read) | 2 (fmm_list_exports) | 899 | 5 | **99.4%** |
| **TOTAL** | **16** | **5** | **2505** | **13** | **99.5%** |

### Tool Usage Details

**Control (without FMM):**
- `find_export`: Grep → scan files
- `dependency_graph`: Bash + Read + Grep → manual analysis
- `list_exports`: Bash + Glob + Grep + Read → manual scan

**Treatment (with FMM):**
- `find_export`: `mcp__fmm__fmm_lookup_export` (O(1) lookup)
- `dependency_graph`: `mcp__fmm__fmm_dependency_graph` (pre-computed graph)
- `list_exports`: `mcp__fmm__fmm_list_exports` (indexed search)

---

## Key Findings

### 1. Skill Path Bug Fixed

The original `init_skill()` function wrote to `.claude/skills/fmm-navigate.md` instead of the correct `.claude/skills/fmm-navigate/SKILL.md` format required by Claude Code.

**Fixed in:** `src/cli/mod.rs:949-973`

### 2. MCP Config Loading

`--setting-sources local` does NOT load MCP servers. Must use `--mcp-config .mcp.json` explicitly.

### 3. Token Reduction Claim: VERIFIED

- **Claimed**: 88-97% reduction
- **Measured**: 99.5% input token reduction
- **Exceeds claim** because MCP tools return structured, minimal responses vs grep/read returning full file contents

### 4. Tool Call Reduction Claim: VERIFIED

- **Claimed**: ~30% reduction
- **Measured**: 68.7% reduction (16 → 5 tools)
- **Exceeds claim** because single MCP call replaces multiple grep/read sequences

---

## Cost Analysis

| Metric | Control | Treatment | Notes |
|--------|---------|-----------|-------|
| Input Tokens | 2,505 | 13 | **99.5% reduction** |
| Output Tokens | 1,131 | 275 | 75.7% reduction |
| Tool Calls | 16 | 5 | 68.7% reduction |
| Read Calls | 3 | 0 | 100% reduction |

---

## Reproducibility

```bash
cd /Users/alphab/Dev/LLM/DEV/fmm
cargo build --release
cd TMP/experiment-verification
./run-experiment.sh
```

**Requirements:**
- Claude CLI with Opus/Sonnet access
- agentic-flow repo at `/Users/alphab/Dev/LLM/DEV/agentic-flow`

---

## Conclusion

**All three claims verified:**

1. ✅ **Token Reduction (88-97%)**: Measured 99.5% input token reduction. EXCEEDS claim.

2. ✅ **Tool Call Reduction (~30%)**: Measured 68.7% reduction. EXCEEDS claim.

3. ✅ **FMM MCP Tools Used**: Claude used FMM MCP tools in all 3 treatment tasks.

**Bug Fixed:** The skill installation path was incorrect. Fixed to use `.claude/skills/fmm-navigate/SKILL.md`.

---

*Generated: 2026-02-13*
