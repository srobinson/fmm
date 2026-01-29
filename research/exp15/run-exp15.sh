#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# exp15: Skill vs CLAUDE.md vs MCP — Full 48-run experiment
# ============================================================================
#
# Usage:
#   ./run-exp15.sh                    # Run everything (48 runs)
#   ./run-exp15.sh A                  # Run condition A only (12 runs)
#   ./run-exp15.sh A 1                # Run condition A, task 1 only (3 runs)
#   ./run-exp15.sh A 1 2              # Run condition A, task 1, run 2 only
#
# Prerequisites:
#   - claude CLI installed and authenticated
#   - fmm installed (cargo install --path /path/to/fmm)
#   - Target repo at $TARGET_REPO
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_REPO="/Users/alphab/Dev/LLM/DEV/agentic-flow"
RESULTS_DIR="$SCRIPT_DIR/results"
SANDBOX_BASE="/tmp/exp15-sandbox"

# Condition filter (optional)
COND_FILTER="${1:-}"
TASK_FILTER="${2:-}"
RUN_FILTER="${3:-}"

# --- Task Prompts -----------------------------------------------------------
# Using real exports from agentic-flow's manifest (3426 exports, 1306 files)

TASK_1="Describe the architecture of this project. What are the main modules and how do they interact?"
TASK_2="Find where the function createBillingSystem is defined and what module it belongs to."
TASK_3="If I change the function signature of validatePasswordStrength, what files would be affected?"
TASK_4="What external packages does this project depend on? List the top 10 by usage."

declare -a TASKS=("$TASK_1" "$TASK_2" "$TASK_3" "$TASK_4")
declare -a TASK_NAMES=("architecture" "export-lookup" "impact-analysis" "dependency-map")
declare -a CONDITIONS=("A" "B" "C" "D")

# --- Setup Functions ---------------------------------------------------------

setup_sandbox() {
    local condition="$1"
    local sandbox="$SANDBOX_BASE/$condition"

    echo "  Setting up sandbox: $sandbox"

    # Fresh copy each time (use rsync to skip node_modules etc)
    rm -rf "$sandbox"
    mkdir -p "$sandbox"
    rsync -a \
        --exclude 'node_modules' \
        --exclude '.git' \
        --exclude 'dist' \
        --exclude '.next' \
        --exclude '.fmm' \
        --exclude '.fmmrc.json' \
        --exclude '.mcp.json' \
        --exclude '.claude/skills/fmm-navigate.md' \
        "$TARGET_REPO/" "$sandbox/"

    # Generate manifest (all conditions need it)
    cd "$sandbox"
    fmm init 2>/dev/null  # creates .fmmrc.json + manifest + skill + mcp

    # Now strip what this condition shouldn't have
    case "$condition" in
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
            echo "    → Condition A: CLAUDE.md snippet added, skill removed, MCP removed"
            ;;
        B)
            # Skill only — keep skill, remove CLAUDE.md fmm content + MCP
            rm -f .mcp.json
            # Don't touch CLAUDE.md (no fmm content since we copied clean)
            echo "    → Condition B: Skill kept, MCP removed"
            ;;
        C)
            # MCP only — keep MCP config, remove skill + CLAUDE.md fmm content
            rm -f .claude/skills/fmm-navigate.md
            echo "    → Condition C: MCP kept, skill removed"
            ;;
        D)
            # Skill + MCP — keep both
            echo "    → Condition D: Skill + MCP (full integration)"
            ;;
    esac

    cd "$SCRIPT_DIR"
}

# --- Run Function ------------------------------------------------------------

run_single() {
    local condition="$1"
    local task_idx="$2"
    local run_num="$3"
    local task_prompt="${TASKS[$task_idx]}"
    local task_name="${TASK_NAMES[$task_idx]}"
    local sandbox="$SANDBOX_BASE/$condition"
    local outfile="$RESULTS_DIR/$condition/${task_name}_run${run_num}.jsonl"

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Condition: $condition | Task: $task_name | Run: $run_num"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Allowed tools depend on condition
    local allowed_tools="Read,Glob,Grep,Bash,LS"
    if [[ "$condition" == "C" || "$condition" == "D" ]]; then
        allowed_tools="$allowed_tools,mcp__fmm__*"
    fi

    # Run claude in the sandbox directory
    cd "$sandbox"

    local start_time
    start_time=$(python3 -c "import time; print(int(time.time()*1000))")

    claude --output-format stream-json --verbose \
        --allowedTools "$allowed_tools" \
        --max-turns 30 \
        -p "$task_prompt" \
        > "$outfile" 2>/dev/null || true

    local end_time
    end_time=$(python3 -c "import time; print(int(time.time()*1000))")
    local duration=$(( end_time - start_time ))

    # Append timing metadata
    echo "{\"_meta\":{\"condition\":\"$condition\",\"task\":\"$task_name\",\"run\":$run_num,\"duration_ms\":$duration}}" >> "$outfile"

    echo "  ✓ Done in ${duration}ms → $outfile"
    cd "$SCRIPT_DIR"
}

# --- Main Loop ---------------------------------------------------------------

echo "╔══════════════════════════════════════════════════════════╗"
echo "║           exp15: Instruction Delivery Comparison        ║"
echo "║                                                         ║"
echo "║  A = CLAUDE.md only    B = Skill only                  ║"
echo "║  C = MCP only          D = Skill + MCP                 ║"
echo "║                                                         ║"
echo "║  4 tasks × 4 conditions × 3 runs = 48 total            ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Target: $TARGET_REPO"
echo "Results: $RESULTS_DIR"
echo ""

total=0
completed=0

# Count total runs
for cond in "${CONDITIONS[@]}"; do
    [[ -n "$COND_FILTER" && "$cond" != "$COND_FILTER" ]] && continue
    for tidx in 0 1 2 3; do
        task_num=$((tidx + 1))
        [[ -n "$TASK_FILTER" && "$task_num" != "$TASK_FILTER" ]] && continue
        for run in 1 2 3; do
            [[ -n "$RUN_FILTER" && "$run" != "$RUN_FILTER" ]] && continue
            total=$((total + 1))
        done
    done
done

echo "Runs planned: $total"
echo ""

for cond in "${CONDITIONS[@]}"; do
    [[ -n "$COND_FILTER" && "$cond" != "$COND_FILTER" ]] && continue

    echo "══════════════════════════════════════════════════════════"
    echo "  CONDITION $cond"
    echo "══════════════════════════════════════════════════════════"

    setup_sandbox "$cond"

    for tidx in 0 1 2 3; do
        task_num=$((tidx + 1))
        [[ -n "$TASK_FILTER" && "$task_num" != "$TASK_FILTER" ]] && continue

        for run in 1 2 3; do
            [[ -n "$RUN_FILTER" && "$run" != "$RUN_FILTER" ]] && continue

            completed=$((completed + 1))
            echo ""
            echo "  [$completed/$total]"
            run_single "$cond" "$tidx" "$run"
        done
    done
done

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  ALL RUNS COMPLETE                                      ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Results in: $RESULTS_DIR"
echo "Next: python3 $SCRIPT_DIR/parse-results.py"
