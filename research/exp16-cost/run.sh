#!/usr/bin/env bash
set -euo pipefail

# exp16: A/B cost experiment
# A = Vanilla Claude (no fmm)
# B = Claude + fmm (headers + MCP + skill)
#
# Measures: tool calls, tokens, correctness, files read

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

source "$SCRIPT_DIR/.env" 2>/dev/null || { echo "Missing .env (needs ANTHROPIC_API_KEY, CODEBASE_PATH)"; exit 1; }
export ANTHROPIC_API_KEY CODEBASE_PATH

CONDITION="${1:-}"
if [[ -z "$CONDITION" ]]; then
  echo "Usage: ./run.sh <A|B|AB> [run_number]"
  echo "  A  = vanilla Claude (no fmm)"
  echo "  B  = Claude + fmm (headers + MCP + skill)"
  echo "  AB = both conditions"
  exit 1
fi

RUN="${2:-1}"
MAX_PARALLEL="${MAX_PARALLEL:-2}"
TASKS_FILE="$SCRIPT_DIR/tasks.json"
TASK_IDS=$(python3 -c "import json; [print(t['id']) for t in json.load(open('$TASKS_FILE'))['tasks']]")

mkdir -p results/A results/B results/.logs

CONDITIONS=()
[[ "$CONDITION" == *A* ]] && CONDITIONS+=(A)
[[ "$CONDITION" == *B* ]] && CONDITIONS+=(B)

echo "╔══════════════════════════════════════════════════╗"
echo "║  exp16: A/B Cost Experiment                      ║"
echo "║                                                  ║"
echo "║  A = Vanilla (no fmm)                           ║"
echo "║  B = fmm (headers + MCP + skill)                ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# Build Docker images
echo "Building Docker images..."
BUILD_SERVICES=()
for C in "${CONDITIONS[@]}"; do
  BUILD_SERVICES+=("condition-$(echo "$C" | tr 'A-Z' 'a-z')")
done
docker compose build "${BUILD_SERVICES[@]}" 2>&1 | tail -5
echo ""

# Count total runs
TOTAL=0
for C in "${CONDITIONS[@]}"; do
  for TASK_ID in $TASK_IDS; do
    TOTAL=$((TOTAL + 1))
  done
done

echo "Runs planned: $TOTAL (${#CONDITIONS[@]} conditions × $(echo "$TASK_IDS" | wc -l | tr -d ' ') tasks × run $RUN)"
echo ""

DONE=0
RUNNING=0
PIDS=()

wait_for_slot() {
  while [[ $RUNNING -ge $MAX_PARALLEL ]]; do
    for i in "${!PIDS[@]}"; do
      if ! kill -0 "${PIDS[$i]}" 2>/dev/null; then
        wait "${PIDS[$i]}" 2>/dev/null || true
        unset 'PIDS[i]'
        RUNNING=$((RUNNING - 1))
      fi
    done
    sleep 2
  done
}

run_one() {
  local COND="$1"
  local TASK_ID="$2"
  local RUN_NUM="$3"
  local OUTFILE="results/${COND}/${TASK_ID}_run${RUN_NUM}.jsonl"
  local LOGFILE="results/.logs/${COND}_${TASK_ID}_run${RUN_NUM}.log"
  local SERVICE="condition-$(echo "$COND" | tr 'A-Z' 'a-z')"

  # Get the prompt for this task
  local PROMPT
  PROMPT=$(python3 -c "
import json
tasks = json.load(open('$TASKS_FILE'))['tasks']
for t in tasks:
    if t['id'] == '$TASK_ID':
        print(t['prompt'])
        break
")

  docker compose run --rm \
    -e ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" \
    -e TASK_PROMPT="$PROMPT" \
    -e CONDITION="$COND" \
    -e TASK_ID="$TASK_ID" \
    "$SERVICE" > "$OUTFILE" 2>"$LOGFILE" || true

  DONE=$((DONE + 1))
  echo "  [$DONE/$TOTAL] Done: $COND / $TASK_ID / run $RUN_NUM"
}

for C in "${CONDITIONS[@]}"; do
  for TASK_ID in $TASK_IDS; do
    wait_for_slot
    echo "  [$((DONE+RUNNING+1))/$TOTAL] Starting: $C / $TASK_ID / run $RUN"
    run_one "$C" "$TASK_ID" "$RUN" &
    PIDS+=($!)
    RUNNING=$((RUNNING + 1))
  done
done

# Wait for remaining
for pid in "${PIDS[@]}"; do
  wait "$pid" 2>/dev/null || true
done

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║  ALL RUNS COMPLETE: $DONE / $TOTAL               "
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "Next: python3 score.py"
