#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# fmm experiment entrypoint — runs a single isolated experiment
# ============================================================================
#
# Args: <condition> <task_idx> <run_num>
#   condition: A | B | C | D
#   task_idx:  0-3 (architecture, export-lookup, impact-analysis, dependency-map)
#   run_num:   1-3
#
# Env:
#   ANTHROPIC_API_KEY — required
#   CODEBASE_DIR     — mounted codebase (default: /codebase)
#   RESULTS_DIR      — output directory (default: /results)

CONDITION="${1:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"
TASK_IDX="${2:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"
RUN_NUM="${3:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"

CODEBASE_DIR="${CODEBASE_DIR:-/codebase}"
RESULTS_DIR="${RESULTS_DIR:-/results}"

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "ERROR: ANTHROPIC_API_KEY not set"
    exit 1
fi

# Task prompts (same as exp15)
TASKS=(
    "Describe the architecture of this project. What are the main modules and how do they interact?"
    "Find where the function createBillingSystem is defined and what module it belongs to."
    "If I change the function signature of validatePasswordStrength, what files would be affected?"
    "What external packages does this project depend on? List the top 10 by usage."
)
TASK_NAMES=("architecture" "export-lookup" "impact-analysis" "dependency-map")

TASK_PROMPT="${TASKS[$TASK_IDX]}"
TASK_NAME="${TASK_NAMES[$TASK_IDX]}"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Condition: $CONDITION | Task: $TASK_NAME | Run: $RUN_NUM"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ─── Setup workspace ─────────────────────────────────────────────────────────
WORKSPACE="/experiment/workspace"
rm -rf "$WORKSPACE"
mkdir -p "$WORKSPACE"

rsync -a \
    --exclude 'node_modules' \
    --exclude '.git' \
    --exclude 'dist' \
    --exclude '.next' \
    --exclude '.fmm' \
    --exclude '.fmmrc.json' \
    --exclude '.mcp.json' \
    --exclude '.claude' \
    "$CODEBASE_DIR/" "$WORKSPACE/"

cd "$WORKSPACE"

# Generate manifest (all conditions need it)
fmm init 2>/dev/null

# ─── Apply condition-specific config ─────────────────────────────────────────
case "$CONDITION" in
    A)
        # CLAUDE.md only — add snippet, remove skill + MCP
        rm -f .claude/skills/fmm-navigate.md
        rm -f .mcp.json
        cat >> CLAUDE.md << 'SNIPPET'

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
SNIPPET
        echo "  → Condition A: CLAUDE.md snippet added, skill removed, MCP removed"
        ;;
    B)
        # Skill only — keep skill, remove MCP
        rm -f .mcp.json
        echo "  → Condition B: Skill kept, MCP removed"
        ;;
    C)
        # MCP only — keep MCP, remove skill
        rm -f .claude/skills/fmm-navigate.md
        echo "  → Condition C: MCP kept, skill removed"
        ;;
    D)
        # Skill + MCP — keep both
        echo "  → Condition D: Skill + MCP (full integration)"
        ;;
    *)
        echo "ERROR: Unknown condition: $CONDITION (expected A, B, C, D)"
        exit 1
        ;;
esac

# ─── Verify isolation ───────────────────────────────────────────────────────
echo ""
echo "  Isolation check:"
echo "    ~/.claude exists: $([ -d ~/.claude ] && echo YES || echo NO)"
echo "    ~/.config exists: $([ -d ~/.config ] && echo YES || echo NO)"
echo "    .mcp.json exists: $([ -f .mcp.json ] && echo YES || echo NO)"
echo "    skill exists:     $([ -f .claude/skills/fmm-navigate.md ] && echo YES || echo NO)"
echo "    manifest exists:  $([ -f .fmm/index.json ] && echo YES || echo NO)"
echo ""

# ─── Run experiment ──────────────────────────────────────────────────────────
OUTDIR="$RESULTS_DIR/$CONDITION"
mkdir -p "$OUTDIR"
OUTFILE="$OUTDIR/${TASK_NAME}_run${RUN_NUM}.jsonl"

# Allowed tools depend on condition
ALLOWED_TOOLS="Read,Glob,Grep,Bash,LS"
if [[ "$CONDITION" == "C" || "$CONDITION" == "D" ]]; then
    ALLOWED_TOOLS="$ALLOWED_TOOLS,mcp__fmm__*"
fi

START_MS=$(python3 -c "import time; print(int(time.time()*1000))")

claude --output-format stream-json --verbose \
    --allowedTools "$ALLOWED_TOOLS" \
    --max-turns 30 \
    --setting-sources "" \
    --no-session-persistence \
    -p "$TASK_PROMPT" \
    > "$OUTFILE" 2>/dev/null || true

END_MS=$(python3 -c "import time; print(int(time.time()*1000))")
DURATION=$(( END_MS - START_MS ))

# Append metadata
echo "{\"_meta\":{\"condition\":\"$CONDITION\",\"task\":\"$TASK_NAME\",\"run\":$RUN_NUM,\"duration_ms\":$DURATION,\"isolated\":true}}" >> "$OUTFILE"

echo "  Done in ${DURATION}ms → $OUTFILE"
echo "  Output size: $(wc -l < "$OUTFILE") lines"
