#!/usr/bin/env bash
# run-navigation.sh — Navigation proof harness
#
# Runs a defined navigation query against control (no fmm) vs treatment (with fmm)
# and captures tool calls, file reads, tokens, and wall-clock time.
#
# Usage:
#   ./harness/run-navigation.sh              # Run all queries
#   ./harness/run-navigation.sh --query 1    # Run specific query
#   ./harness/run-navigation.sh --dry-run    # Show what would run
#
# Prerequisites: claude CLI, python3
# Run from: proofs/ directory (or it auto-detects)
set -euo pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROOFS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$PROOFS_DIR/.." && pwd)"

CONTROL_REPO="$PROJECT_ROOT/research/exp14/repos/clean"
TREATMENT_REPO="$PROJECT_ROOT/research/exp14/repos/hint"

CONTROL_OUT="$PROOFS_DIR/content/control"
TREATMENT_OUT="$PROOFS_DIR/content/treatment"

MODEL="${PROOF_MODEL:-sonnet}"
MAX_BUDGET="${PROOF_BUDGET:-1.00}"

# Navigation queries — pure "understand the codebase" tasks
QUERIES=(
  "Describe the architecture of this project. What are the main modules, their roles, key exports, and how they depend on each other? Be specific about file paths."
  "Which file defines the createApp function? What does it depend on? Trace the full dependency chain."
  "Find all files that export authentication-related functions. List each file path and the specific auth exports."
)

QUERY_LABELS=(
  "architecture-overview"
  "export-trace"
  "auth-exports"
)

# ---------------------------------------------------------------------------
# CLI args
# ---------------------------------------------------------------------------
QUERY_FILTER=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --query)  QUERY_FILTER="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --model)  MODEL="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
if ! command -v claude &>/dev/null; then
  echo "ERROR: 'claude' CLI not found. Install Claude Code first." >&2
  exit 1
fi

if ! command -v python3 &>/dev/null; then
  echo "ERROR: python3 not found." >&2
  exit 1
fi

if [ ! -d "$CONTROL_REPO" ]; then
  echo "ERROR: Control repo not found at $CONTROL_REPO" >&2
  exit 1
fi

if [ ! -d "$TREATMENT_REPO" ]; then
  echo "ERROR: Treatment repo not found at $TREATMENT_REPO" >&2
  exit 1
fi

mkdir -p "$CONTROL_OUT" "$TREATMENT_OUT"

# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------
run_query() {
  local condition="$1"  # control | treatment
  local repo_dir="$2"
  local query="$3"
  local label="$4"
  local query_num="$5"

  local out_dir
  if [ "$condition" = "control" ]; then
    out_dir="$CONTROL_OUT"
  else
    out_dir="$TREATMENT_OUT"
  fi

  local raw_file="$out_dir/${label}_raw.jsonl"
  local result_file="$out_dir/${label}.json"
  local transcript_file="$out_dir/${label}_transcript.txt"

  echo "  [$condition] Query $query_num: $label"
  echo "  Repo: $repo_dir"

  if $DRY_RUN; then
    echo "  (dry run — skipping)"
    return
  fi

  local system_prompt="You are a coding assistant. Help the developer navigate and understand this codebase. Use available tools to explore files and directories. Be thorough and accurate."

  # Treatment gets fmm navigation instructions via --append-system-prompt
  # This simulates what happens when a project has CLAUDE.md with fmm guidance
  local fmm_hint=""
  if [ "$condition" = "treatment" ]; then
    fmm_hint="This project uses fmm metadata. Check .fmm/index.json FIRST before reading source files. The manifest contains exports, imports, dependencies, and LOC for every file — use it to navigate without opening source."
  fi

  # Empty MCP config to prevent ambient MCP servers
  local empty_mcp
  empty_mcp=$(mktemp)
  echo '{"mcpServers":{}}' > "$empty_mcp"

  local start_time
  start_time=$(date +%s)

  # Build Claude CLI command
  local -a claude_args=(
    --print
    --verbose
    --no-session-persistence
    --dangerously-skip-permissions
    --output-format stream-json
    --system-prompt "$system_prompt"
    --model "$MODEL"
    --setting-sources ""
    --strict-mcp-config
    --mcp-config "$empty_mcp"
    --disable-slash-commands
    --tools "Bash,Read,Glob,Grep"
    --max-budget-usd "$MAX_BUDGET"
  )

  if [ -n "$fmm_hint" ]; then
    claude_args+=(--append-system-prompt "$fmm_hint")
  fi

  # Run Claude from within the repo directory
  (
    cd "$repo_dir"
    claude "${claude_args[@]}" "$query" > "$raw_file" 2>/dev/null
  ) || true

  rm -f "$empty_mcp"

  local end_time
  end_time=$(date +%s)
  local duration=$((end_time - start_time))

  echo "  Completed in ${duration}s"

  # Parse stream-json output into structured metrics
  python3 "$SCRIPT_DIR/parse-results.py" \
    "$raw_file" "$result_file" "$transcript_file" \
    "$condition" "$label" "$duration" "$query"

  echo "  Results: $result_file"
  echo ""
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║          FMM Navigation Proof Harness                      ║"
echo "╠══════════════════════════════════════════════════════════════╣"
printf "║  Model: %-50s║\n" "$MODEL"
printf "║  Control:   %-47s║\n" "$(basename "$CONTROL_REPO")/ (no fmm)"
printf "║  Treatment: %-47s║\n" "$(basename "$TREATMENT_REPO")/ (fmm + CLAUDE.md)"
printf "║  Queries: %-49s║\n" "${QUERY_FILTER:-all (${#QUERIES[@]})}"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

TOTAL_START=$(date +%s)

for i in "${!QUERIES[@]}"; do
  query_num=$((i + 1))

  if [ -n "$QUERY_FILTER" ] && [ "$QUERY_FILTER" != "$query_num" ]; then
    continue
  fi

  echo "━━━ Query $query_num: ${QUERY_LABELS[$i]} ━━━"
  echo "  \"${QUERIES[$i]:0:80}...\""
  echo ""

  run_query "control"   "$CONTROL_REPO"   "${QUERIES[$i]}" "${QUERY_LABELS[$i]}" "$query_num"
  run_query "treatment" "$TREATMENT_REPO" "${QUERIES[$i]}" "${QUERY_LABELS[$i]}" "$query_num"
done

TOTAL_END=$(date +%s)
TOTAL_DURATION=$((TOTAL_END - TOTAL_START))

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "All runs complete in ${TOTAL_DURATION}s"
echo ""

# Generate comparison summary
if ! $DRY_RUN; then
  echo "Generating comparison summary..."
  python3 "$SCRIPT_DIR/compare-results.py" \
    "$CONTROL_OUT" "$TREATMENT_OUT" \
    "$PROOFS_DIR/stats/summary.md"
  echo "Summary written to: proofs/stats/summary.md"
fi
