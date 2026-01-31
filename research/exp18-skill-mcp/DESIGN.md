# Exp18: Sidecar + Skill + MCP on Kysely Implementation Task

## Hypothesis

Sidecars + Skill + MCP on a large repo (Kysely, 471 files) will reduce tool calls
by 20-40% and cost by 30-50% on an implementation task, compared to the -2% to -8%
seen in Exp17/17b which used preamble-only delivery (the weakest mechanism).

## Why This Experiment Matters

Exp17/17b tested sidecars with `--setting-sources ""` which disabled both skills AND
MCP — leaving only a `--append-system-prompt` preamble. Exp15 proved this is the LEAST
effective delivery mechanism. We've never tested sidecars with the proven-best mechanism
(Skill + MCP combined).

## Design

### Task
"Implement DROP COLUMN IF EXISTS for the ALTER TABLE dialect"
(Same as Exp17 for direct comparison)

### Conditions

| Condition | Sidecars | Skill | MCP | Settings |
|-----------|----------|-------|-----|----------|
| A: Control | No | No | No | `--setting-sources ""` |
| B: Sidecar + Skill + MCP | Yes | Yes | Yes | `--setting-sources local` |

### Setup

**Control (A):**
- Clean Kysely clone (no .fmm files, no .claude/, no .mcp.json)
- `--setting-sources ""` (fully isolated)

**Treatment (B):**
- Kysely clone + `fmm generate .` (sidecars)
- `fmm init --all` (installs .claude/skills/fmm-navigate.md + .mcp.json)
- `--setting-sources local` (picks up local skill + MCP only)

### Model
Sonnet (same as Exp17 for comparison)

### Runs
3 per condition (variance matters on this task)

### Metrics
- Tool calls (total and by type: Read, Glob, Grep, fmm_*)
- Files read count
- Sidecar reads vs source reads (B only)
- Cost (USD)
- Wall time
- Whether MCP tools were invoked (B only)
- Whether skill instructions were followed (B only: first action = sidecar grep?)
- Correctness: does the implementation compile and pass relevant tests?

### Read Classification (from ALP-407)
For each Read call, classify as:
- **Exploration**: File was read but NOT subsequently edited
- **Pre-edit**: File was read AND then edited
- **Reference**: File was read for pattern/context related to edit target
- **Sidecar**: .fmm file read (B only)

## Expected Results

Based on Exp15 data (30% fewer tool calls with Skill+MCP) and the bounded
implementation savings insight (-20% to -40%):

| Metric | Control (A) | Treatment (B) | Expected Delta |
|--------|-------------|---------------|----------------|
| Tool calls | ~12 (Exp17 baseline) | 7-10 | -20% to -40% |
| Cost | ~$0.30 | $0.15-0.21 | -30% to -50% |
| Files read | ~12 | 6-10 | -15% to -50% |
| MCP calls | 0 | 2-5 | N/A (new capability) |

## Differences from Exp17

| Aspect | Exp17 | Exp18 |
|--------|-------|-------|
| Delivery mechanism | Preamble only | Skill + MCP |
| `--setting-sources` | `""` (disabled all) | `local` (workspace only) |
| Skill file | Not installed | .claude/skills/fmm-navigate.md |
| MCP server | Disabled | fmm serve via .mcp.json |
| Expected reduction | -2% to -8% | -20% to -40% |

## Execution

### Method 1: `fmm compare` CLI (recommended)
```bash
fmm compare https://github.com/kysely-org/kysely \
  --tasks research/exp18-skill-mcp/tasks.json \
  --runs 3 \
  --output research/exp18-skill-mcp/results \
  --model sonnet \
  --max-budget 10.0
```

### Method 2: Manual script
```bash
cd research/exp18-skill-mcp
./run.sh
```

## Success Criteria

1. Treatment shows >= 20% fewer tool calls than control (averaged across runs)
2. MCP tools are invoked in >= 2/3 treatment runs
3. Skill instructions are followed (first exploration action uses sidecars)
4. Cost reduction >= 30%
5. Correctness is equivalent between conditions

## Failure Criteria

If treatment shows < 10% improvement or shows regression, the hypothesis is
falsified and sidecars don't help even with proper delivery. In that case, the
remaining value proposition is navigation/lookup only (where we know it works).
