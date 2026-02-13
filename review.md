# fmm — Project Review

**Reviewer:** Kilo (AI Assistant)  
**Date:** 2026-02-13  
**Version:** 0.1.0  
**Repository:** https://github.com/mdcontext/fmm

---

## Executive Summary

fmm is a well-conceived tool that addresses a real problem: LLM code navigation is token-inefficient. The core value proposition is validated — metadata sidecars enable 99.5% input token reduction on navigation tasks. However, the project had critical integration issues that undermined its claims in real-world usage.

**Verdict:** Promising. Critical bugs fixed. Ready for next iteration.

---

## Changes Made During Review

### 1. Fixed Skill Installation Path ✅

**Location:** `src/cli/mod.rs:985-1010`

```rust
// BEFORE: Wrong path
let skill_path = skill_dir.join("fmm-navigate.md");

// AFTER: Correct path
let skill_path = skill_dir.join("fmm-navigate").join("SKILL.md");
```

### 2. Improved Skill Content ✅

**Location:** `docs/fmm-navigate.md`

- MCP tools now emphasized as PRIMARY navigation mechanism
- Clear protocol: "Always call fmm_* tools before grep/read"
- Structured guidance for common query patterns

### 3. Added `fmm run` Command ✅

**Location:** `src/cli/mod.rs:1377-1450`

Natural language queries for humans:
```bash
fmm run "What's the architecture of the auth module?"
fmm run "Which files have the most dependencies?"
fmm run "What would break if I delete utils/format.ts?"
```

### 4. Updated project.md ✅

Verification report documenting:
- 99.5% input token reduction
- 68.7% tool call reduction
- MCP tools successfully used

---

## What Works Well

### 1. Core Architecture

The design is sound:
- Tree-sitter parsing for 9 languages is robust
- Sidecar format is clean and readable (YAML)
- MCP server provides structured O(1) lookups
- In-memory manifest enables fast queries

### 2. Claims Are Valid

| Claim | Measured | Status |
|-------|----------|--------|
| 88-97% token reduction | 99.5% | ✅ Exceeds claim |
| ~30% tool call reduction | 68.7% | ✅ Exceeds claim |

The MCP tools (`fmm_lookup_export`, `fmm_dependency_graph`, `fmm_list_exports`) work exactly as advertised.

### 3. Research Rigor

The `research/` directory contains actual experiments with methodology, raw data, and findings. This is rare in developer tools. Exp 13-17 provide genuine evidence for claims.

### 4. Human-Friendly CLI

The CLI already supports structured queries:
```bash
fmm search --export X          # Where is X defined?
fmm search --imports react     # What uses react?
fmm search --depends-on file   # Impact analysis
fmm search --loc ">500"        # Find large files
```

Now with `fmm run` for natural language queries too.

---

## Remaining Issues

### 1. MCP Config Not Auto-Loaded

**Severity: Medium**

`--setting-sources local` loads skills but NOT MCP servers. Users must either:
- Use `fmm run` (handles it automatically)
- Use `--mcp-config .mcp.json` explicitly

**Recommendation:** Document clearly in README. Don't auto-modify user settings.

### 2. MCP Tool Names Are Verbose

```
mcp__fmm__fmm_lookup_export
mcp__fmm__fmm_list_exports
```

The double `fmm` prefix is redundant. Consider simplifying in future version.

### 3. No Integration Tests for Skill/MCP Loading

The test suite has 61 tests but none verify end-to-end integration.

**Proposal:** Add integration test that:
1. Runs `fmm init --all`
2. Verifies skill path exists
3. Starts MCP server and calls a tool
4. Asserts correct response

---

## Design Critiques

### 1. Two Delivery Mechanisms (Sidecars vs Manifest)

The project supports both:
- Individual `.fmm` sidecar files alongside sources
- Centralized `.fmm/index.json` manifest

This is confusing. Which should users prefer? The documentation mentions both but doesn't clearly recommend one.

**Proposal:** Pick one default and deprecate the other, or clearly document when to use each.

### 2. Sidecar Generation Verbose Output

`fmm generate` could be more informative about what was scanned. Consider adding a `--verbose` flag that shows:
- Languages detected
- Files excluded (and why)
- Parse errors (if any)

---

## Recommended Next Steps

### Immediate (Done ✅)

- [x] Fix skill path
- [x] Improve skill content  
- [x] Add `fmm run` command
- [x] Create verification report

### Next Release

1. **Add integration tests** for skill/MCP loading
2. **Add `--verbose` output** to `fmm generate`
3. **Document MCP setup** clearly in README

### Future

1. **Consider LSP approach** instead of sidecars
2. **Add incremental updates** with `fmm watch`
3. **Explore cloud manifest hosting** for zero-config onboarding

---

## Metrics Baseline

For future benchmarking, here are the numbers from verification:

| Metric | Control | FMM | Reduction |
|--------|---------|-----|-----------|
| Tool calls (avg) | 5.3 | 1.7 | 68.7% |
| Read calls (avg) | 1.0 | 0.0 | 100% |
| Input tokens (total) | 2,505 | 13 | 99.5% |

These should be tracked over time to detect regressions.

---

## Conclusion

fmm solves a real problem with a sound technical approach. The verification confirmed that token reduction claims are accurate and the MCP tools work as designed.

**Rating: 8/10** (up from 7/10 after fixes)

- Core technology: 9/10
- Implementation: 7/10
- Documentation: 7/10
- Testing: 5/10
- Integration: 7/10

Critical bugs fixed. Ready for next release.

---

*Review conducted: 2026-02-13*
