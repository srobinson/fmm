#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CODEBASE="/Users/alphab/Dev/LLM/DEV/agentic-flow"
RESULTS_DIR="$SCRIPT_DIR/results"
TASKS_FILE="$SCRIPT_DIR/tasks.json"
MAX_TURNS=5
CONCURRENCY=2

# Ensure results dirs
rm -rf "$RESULTS_DIR"
mkdir -p "$RESULTS_DIR/A" "$RESULTS_DIR/B"

# Backup original files
cp "$CODEBASE/CLAUDE.md" "$SCRIPT_DIR/.claude-md-backup"
cp "$CODEBASE/.mcp.json" "$SCRIPT_DIR/.mcp-json-backup"

cleanup() {
    echo "Restoring original files..."
    cp "$SCRIPT_DIR/.claude-md-backup" "$CODEBASE/CLAUDE.md"
    cp "$SCRIPT_DIR/.mcp-json-backup" "$CODEBASE/.mcp.json"
}
trap cleanup EXIT

# Extract task IDs and prompts
TASK_IDS=($(python3 -c "
import json
tasks = json.load(open('$TASKS_FILE'))['tasks']
for t in tasks:
    print(t['id'])
"))

TOTAL_TASKS=${#TASK_IDS[@]}

echo "╔══════════════════════════════════════════════════╗"
echo "║  exp17: CLI A/B Cost Experiment                  ║"
echo "║                                                  ║"
echo "║  A = Vanilla (no fmm, no CLAUDE.md)             ║"
echo "║  B = fmm (CLAUDE.md + MCP)                      ║"
echo "║  Codebase: agentic-flow                          ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "Tasks: $TOTAL_TASKS per condition = $((TOTAL_TASKS * 2)) total runs"
echo ""

run_task() {
    local cond="$1"
    local task_id="$2"
    local prompt
    prompt=$(python3 -c "
import json, sys
tasks = json.load(open('$TASKS_FILE'))['tasks']
for t in tasks:
    if t['id'] == '$task_id':
        print(t['prompt'])
        sys.exit(0)
sys.exit(1)
")

    local outfile="$RESULTS_DIR/$cond/${task_id}_run1.jsonl"

    echo "  [$cond] Starting: $task_id"

    cd "$CODEBASE"
    claude -p "$prompt" \
        --output-format stream-json \
        --verbose \
        --max-turns "$MAX_TURNS" \
        --dangerously-skip-permissions \
        > "$outfile" 2>&1

    local exit_code=$?
    cd "$SCRIPT_DIR"

    if [ $exit_code -eq 0 ]; then
        echo "  [$cond] Done: $task_id"
    else
        echo "  [$cond] FAILED (exit $exit_code): $task_id"
    fi
}

# ── CONDITION A: Vanilla (no fmm) ──────────────────────────────
echo "═══ Condition A: Vanilla (no fmm) ═══"
echo ""

# Remove fmm integration
echo '{}' > "$CODEBASE/.mcp.json"
cat > "$CODEBASE/CLAUDE.md" << 'VANILLA_EOF'
## Code Navigation

Use standard tools (Grep, Glob, Read) to explore the codebase.
VANILLA_EOF

# Run tasks sequentially (avoids cache interference between tasks)
for task_id in "${TASK_IDS[@]}"; do
    run_task "A" "$task_id"
done

echo ""
echo "═══ Condition B: fmm (CLAUDE.md + MCP) ═══"
echo ""

# Restore fmm integration
cp "$SCRIPT_DIR/.mcp-json-backup" "$CODEBASE/.mcp.json"
cp "$SCRIPT_DIR/.claude-md-backup" "$CODEBASE/CLAUDE.md"

for task_id in "${TASK_IDS[@]}"; do
    run_task "B" "$task_id"
done

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║  ALL RUNS COMPLETE                               ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "Next: cd $SCRIPT_DIR && python3 score.py"
