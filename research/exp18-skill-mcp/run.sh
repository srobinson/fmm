#!/usr/bin/env bash
set -euo pipefail

# Exp18: Sidecar + Skill + MCP on Kysely implementation task
# Tests the proven-best delivery mechanism (Exp15) on a large repo implementation task.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
KYSELY_URL="https://github.com/kysely-org/kysely"
SANDBOX_DIR="/tmp/fmm-exp18"
RUNS=3
MODEL="sonnet"
MAX_TURNS=30
MAX_BUDGET="3.0"

TASK_PROMPT="Implement support for DROP COLUMN IF EXISTS in the ALTER TABLE builder. The Kysely query builder currently supports dropping columns, but doesn't support the IF EXISTS modifier. Add an ifExists() modifier that can be chained after dropColumn() in the alter table builder. This should generate SQL like: ALTER TABLE foo DROP COLUMN IF EXISTS bar. Look at how existing ALTER TABLE operations are implemented and follow the same patterns."

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Exp18: Sidecar + Skill + MCP on Kysely                     ║"
echo "║                                                              ║"
echo "║  A = Control (clean, fully isolated)                         ║"
echo "║  B = Sidecar + Skill + MCP (local settings)                 ║"
echo "║  Task: DROP COLUMN IF EXISTS                                 ║"
echo "║  Runs: $RUNS per condition                                   ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Setup
rm -rf "$SANDBOX_DIR" "$RESULTS_DIR"
mkdir -p "$SANDBOX_DIR/control" "$SANDBOX_DIR/fmm"
mkdir -p "$RESULTS_DIR/A" "$RESULTS_DIR/B"

# Clone Kysely for both conditions
echo "Cloning Kysely..."
git clone --depth 1 "$KYSELY_URL" "$SANDBOX_DIR/control" 2>/dev/null
git clone --depth 1 "$KYSELY_URL" "$SANDBOX_DIR/fmm" 2>/dev/null
echo "Done."

# Setup FMM variant: sidecars + skill + MCP
echo "Setting up FMM variant..."
(
    cd "$SANDBOX_DIR/fmm"
    fmm generate .
    fmm init --all
)
SIDECAR_COUNT=$(find "$SANDBOX_DIR/fmm" -name "*.fmm" | wc -l | tr -d ' ')
echo "  $SIDECAR_COUNT sidecar files generated"
echo "  Skill + MCP config installed"
echo ""

run_task() {
    local cond="$1"
    local run_num="$2"
    local working_dir="$3"
    local outfile="$RESULTS_DIR/$cond/drop_column_if_exists_run${run_num}.jsonl"

    echo "  [$cond] Run $run_num/$RUNS..."

    local -a claude_args=(
        -p "$TASK_PROMPT"
        --output-format stream-json
        --verbose
        --max-turns "$MAX_TURNS"
        --max-budget-usd "$MAX_BUDGET"
        --model "$MODEL"
        --dangerously-skip-permissions
        --no-session-persistence
    )

    if [ "$cond" = "A" ]; then
        # Control: fully isolated
        claude_args+=(--setting-sources "")
    else
        # FMM: local settings (skill + MCP from workspace)
        claude_args+=(--setting-sources "local")
    fi

    local start_time
    start_time=$(date +%s)

    (cd "$working_dir" && claude "${claude_args[@]}" > "$outfile" 2>&1) || true

    local end_time
    end_time=$(date +%s)
    local duration=$((end_time - start_time))
    echo "  [$cond] Run $run_num done (${duration}s)"

    # Reset git state for next run
    (cd "$working_dir" && git checkout . 2>/dev/null && git clean -fd 2>/dev/null) || true
    # Re-generate sidecars + skill + MCP for FMM condition (git clean removes them all)
    if [ "$cond" = "B" ]; then
        (cd "$working_dir" && fmm generate . 2>/dev/null && fmm init --all 2>/dev/null) || true
    fi
}

# Condition A: Control
echo "═══ Condition A: Control (no fmm) ═══"
for run in $(seq 1 "$RUNS"); do
    run_task "A" "$run" "$SANDBOX_DIR/control"
done

echo ""

# Condition B: Sidecar + Skill + MCP
echo "═══ Condition B: Sidecar + Skill + MCP ═══"
for run in $(seq 1 "$RUNS"); do
    run_task "B" "$run" "$SANDBOX_DIR/fmm"
done

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  ALL RUNS COMPLETE                                           ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Results in: $RESULTS_DIR"
echo "Next: python3 $SCRIPT_DIR/analyze.py"
