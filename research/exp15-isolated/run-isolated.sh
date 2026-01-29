#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# exp15-isolated: Orchestrate 48 fully isolated Docker experiment runs
# ============================================================================
#
# Usage:
#   ./run-isolated.sh                    # All 48 runs
#   ./run-isolated.sh A                  # Condition A only (12 runs)
#   ./run-isolated.sh A 1               # Condition A, task 1 only (3 runs)
#   ./run-isolated.sh A 1 2             # Single run: Condition A, task 1, run 2
#
# Environment:
#   MAX_PARALLEL  — max concurrent containers (default: 2)
#   CODEBASE_PATH — path to agentic-flow repo (default from .env)
#
# Prerequisites:
#   - Docker + Docker Compose
#   - .env file with ANTHROPIC_API_KEY and CODEBASE_PATH
#   - fmm source copied to fmm-src/ (run: ./setup.sh)
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

MAX_PARALLEL="${MAX_PARALLEL:-2}"
COND_FILTER="${1:-}"
TASK_FILTER="${2:-}"
RUN_FILTER="${3:-}"

CONDITIONS=(A B C D)
TASK_NAMES=("architecture" "export-lookup" "impact-analysis" "dependency-map")

# ─── Preflight checks ───────────────────────────────────────────────────────
if [[ ! -f .env ]]; then
    echo "ERROR: .env file not found. Copy .env.example to .env and configure it."
    exit 1
fi

source .env

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "ERROR: ANTHROPIC_API_KEY not set in .env"
    exit 1
fi

if [[ -z "${CODEBASE_PATH:-}" ]]; then
    echo "ERROR: CODEBASE_PATH not set in .env"
    exit 1
fi

if [[ ! -d "${CODEBASE_PATH}" ]]; then
    echo "ERROR: CODEBASE_PATH does not exist: $CODEBASE_PATH"
    exit 1
fi

# ─── Build image ─────────────────────────────────────────────────────────────
echo "╔══════════════════════════════════════════════════════════╗"
echo "║     exp15-isolated: Docker-based Experiment Runner      ║"
echo "║                                                         ║"
echo "║  A = CLAUDE.md only    B = Skill only                  ║"
echo "║  C = MCP only          D = Skill + MCP                 ║"
echo "║                                                         ║"
echo "║  4 tasks × 4 conditions × 3 runs = 48 total            ║"
echo "║  Max parallel: $MAX_PARALLEL                                       ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Ensure fmm source is available for Docker build
if [[ ! -d fmm-src/src ]]; then
    echo "Setting up fmm source for Docker build..."
    ./setup.sh
fi

echo "Building Docker image..."
docker compose build --quiet condition-a
echo "  Image built successfully."
echo ""

# ─── Enumerate runs ──────────────────────────────────────────────────────────
declare -a JOBS=()

for cond in "${CONDITIONS[@]}"; do
    [[ -n "$COND_FILTER" && "$cond" != "$COND_FILTER" ]] && continue
    for tidx in 0 1 2 3; do
        task_num=$((tidx + 1))
        [[ -n "$TASK_FILTER" && "$task_num" != "$TASK_FILTER" ]] && continue
        for run in 1 2 3; do
            [[ -n "$RUN_FILTER" && "$run" != "$RUN_FILTER" ]] && continue
            JOBS+=("$cond:$tidx:$run")
        done
    done
done

TOTAL=${#JOBS[@]}
echo "Runs planned: $TOTAL"
echo ""

if [[ $TOTAL -eq 0 ]]; then
    echo "No runs match the filter. Exiting."
    exit 0
fi

# ─── Run with concurrency control ───────────────────────────────────────────
COMPLETED=0
FAILED=0
ACTIVE_PIDS=()

cleanup() {
    echo ""
    echo "Interrupted. Stopping running containers..."
    for pid in "${ACTIVE_PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    echo "Stopped."
    exit 1
}
trap cleanup INT TERM

run_job() {
    local job="$1"
    local job_num="$2"
    IFS=':' read -r cond tidx run <<< "$job"
    local task_name="${TASK_NAMES[$tidx]}"
    local service="condition-$(echo "$cond" | tr '[:upper:]' '[:lower:]')"
    local logfile="results/.logs/${cond}_${task_name}_run${run}.log"

    mkdir -p results/.logs

    echo "  [$job_num/$TOTAL] Starting: $cond / $task_name / run $run"

    # Each run gets a unique container name to avoid conflicts
    local container_name="exp15-${cond}-${task_name}-run${run}-$$"

    if docker compose run --rm \
        --name "$container_name" \
        "$service" "$tidx" "$run" \
        > "$logfile" 2>&1; then
        echo "  [$job_num/$TOTAL] Done: $cond / $task_name / run $run"
        return 0
    else
        echo "  [$job_num/$TOTAL] FAILED: $cond / $task_name / run $run (see $logfile)"
        return 1
    fi
}

JOB_NUM=0
for job in "${JOBS[@]}"; do
    JOB_NUM=$((JOB_NUM + 1))

    # Wait if at capacity
    while [[ ${#ACTIVE_PIDS[@]} -ge $MAX_PARALLEL ]]; do
        NEW_PIDS=()
        for pid in "${ACTIVE_PIDS[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
                NEW_PIDS+=("$pid")
            else
                wait "$pid" 2>/dev/null && COMPLETED=$((COMPLETED + 1)) || FAILED=$((FAILED + 1))
            fi
        done
        ACTIVE_PIDS=("${NEW_PIDS[@]}")
        [[ ${#ACTIVE_PIDS[@]} -ge $MAX_PARALLEL ]] && sleep 2
    done

    run_job "$job" "$JOB_NUM" &
    ACTIVE_PIDS+=($!)
done

# Wait for remaining jobs
for pid in "${ACTIVE_PIDS[@]}"; do
    wait "$pid" 2>/dev/null && COMPLETED=$((COMPLETED + 1)) || FAILED=$((FAILED + 1))
done

# ─── Summary ─────────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  ALL RUNS COMPLETE                                      ║"
echo "║  Completed: $COMPLETED / $TOTAL                                      ║"
if [[ $FAILED -gt 0 ]]; then
echo "║  Failed: $FAILED (check results/.logs/)                      ║"
fi
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Results in: $SCRIPT_DIR/results/"
echo "Logs in:    $SCRIPT_DIR/results/.logs/"
echo ""
echo "Next: python3 ../exp15/parse-results.py  (point at results/)"
echo "  or: python3 compare-isolated.py"
