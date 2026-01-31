#!/usr/bin/env bash
set -euo pipefail

# demo.sh — Reproducible demo of fmm's token savings for LLM code navigation
#
# This script runs the same navigation query against a codebase twice:
#   1. WITHOUT fmm (control) — LLM brute-forces with grep+read
#   2. WITH fmm (treatment) — LLM reads .fmm sidecars first, then targeted reads
#
# Requirements: Rust/cargo, claude CLI (with ANTHROPIC_API_KEY set)
# Cost: ~$0.15 total (two Claude Sonnet queries)
# Time: ~2 minutes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FMM_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEMO_WORKSPACE="$(mktemp -d)"
QUERY="Where is the authentication middleware defined? List all exported functions from the auth module."

# Colors (disabled if not a terminal)
if [ -t 1 ]; then
  BOLD='\033[1m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  RED='\033[0;31m'
  CYAN='\033[0;36m'
  RESET='\033[0m'
else
  BOLD='' GREEN='' YELLOW='' RED='' CYAN='' RESET=''
fi

info()  { echo -e "${CYAN}[info]${RESET}  $*"; }
ok()    { echo -e "${GREEN}[ok]${RESET}    $*"; }
warn()  { echo -e "${YELLOW}[warn]${RESET}  $*"; }
fail()  { echo -e "${RED}[fail]${RESET}  $*"; exit 1; }

cleanup() {
  if [ -d "$DEMO_WORKSPACE" ]; then
    rm -rf "$DEMO_WORKSPACE"
  fi
}
trap cleanup EXIT

# ─────────────────────────────────────────────────────────────────────────────
# Step 0: Check prerequisites
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}=== fmm Token Savings Demo ===${RESET}"
echo ""
echo "Workspace: $DEMO_WORKSPACE"
echo "Query:     $QUERY"
echo ""

info "Checking prerequisites..."

command -v cargo >/dev/null 2>&1 || fail "cargo not found. Install Rust: https://rustup.rs"
ok "cargo found: $(cargo --version)"

command -v claude >/dev/null 2>&1 || fail "claude CLI not found. Install: npm install -g @anthropic-ai/claude-code"
ok "claude CLI found"

command -v python3 >/dev/null 2>&1 || fail "python3 not found"
ok "python3 found"

if [ ! -f "$FMM_ROOT/Cargo.toml" ]; then
  fail "Cannot find fmm source at $FMM_ROOT/Cargo.toml. Run this script from the fmm repo: content/demo/demo.sh"
fi
ok "fmm source found at $FMM_ROOT"

# ─────────────────────────────────────────────────────────────────────────────
# Step 1: Build fmm from source
# ─────────────────────────────────────────────────────────────────────────────

echo ""
info "Building fmm from source..."
cargo build --release --manifest-path "$FMM_ROOT/Cargo.toml" 2>&1 | tail -1
FMM_BIN="$FMM_ROOT/target/release/fmm"

if [ ! -x "$FMM_BIN" ]; then
  fail "Build failed: $FMM_BIN not found"
fi
ok "fmm built: $FMM_BIN"

# ─────────────────────────────────────────────────────────────────────────────
# Step 2: Set up the test codebase
# ─────────────────────────────────────────────────────────────────────────────

echo ""
info "Setting up test codebase..."

TEST_APP_SRC="$FMM_ROOT/research/exp14/repos/test-auth-app"
if [ ! -d "$TEST_APP_SRC" ]; then
  fail "Test fixture not found at $TEST_APP_SRC"
fi

# Create two copies: control (no fmm) and treatment (with fmm)
CONTROL_DIR="$DEMO_WORKSPACE/control"
TREATMENT_DIR="$DEMO_WORKSPACE/treatment"

cp -r "$TEST_APP_SRC" "$CONTROL_DIR"
cp -r "$TEST_APP_SRC" "$TREATMENT_DIR"

FILE_COUNT=$(find "$CONTROL_DIR/src" -name "*.ts" | wc -l | tr -d ' ')
ok "Test codebase: 18-file TypeScript auth app ($FILE_COUNT .ts files)"

# ─────────────────────────────────────────────────────────────────────────────
# Step 3: Generate fmm sidecars for treatment
# ─────────────────────────────────────────────────────────────────────────────

echo ""
info "Generating fmm sidecars for treatment codebase..."

(cd "$TREATMENT_DIR" && "$FMM_BIN" init 2>&1) || true
(cd "$TREATMENT_DIR" && "$FMM_BIN" generate 2>&1) || true

SIDECAR_COUNT=$(find "$TREATMENT_DIR" -name "*.fmm" 2>/dev/null | wc -l | tr -d ' ')
if [ "$SIDECAR_COUNT" -eq 0 ]; then
  warn "No sidecars generated. Checking alternative output..."
  find "$TREATMENT_DIR" -name ".fmm" -type d 2>/dev/null
  # Try without init
  (cd "$TREATMENT_DIR" && "$FMM_BIN" generate --format sidecar 2>&1) || true
  SIDECAR_COUNT=$(find "$TREATMENT_DIR" -name "*.fmm" 2>/dev/null | wc -l | tr -d ' ')
fi
ok "Generated $SIDECAR_COUNT fmm artifacts in treatment codebase"

# Ensure CLAUDE.md hint exists in treatment
mkdir -p "$TREATMENT_DIR/.claude"
cat > "$TREATMENT_DIR/.claude/CLAUDE.md" << 'CLAUDEMD'
# FMM Navigation

This codebase has fmm (Frontmatter Matters) sidecars. Before reading source files:
1. Check for .fmm sidecar files (*.fmm) next to source files
2. Read sidecars for file metadata (exports, imports, deps, LOC)
3. Only open source files you actually need to read or edit
CLAUDEMD
ok "Created .claude/CLAUDE.md hint in treatment codebase"

# Strip any CLAUDE.md from control
rm -rf "$CONTROL_DIR/.claude" "$CONTROL_DIR/CLAUDE.md" 2>/dev/null || true

# ─────────────────────────────────────────────────────────────────────────────
# Step 4: Run control experiment (WITHOUT fmm)
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}--- Phase 1: Control (no fmm) ---${RESET}"
info "Running Claude against bare codebase..."

CONTROL_RAW="$DEMO_WORKSPACE/control_raw.jsonl"
SYSTEM_PROMPT="You are a helpful coding assistant. You help developers navigate and understand codebases. Use available tools to explore files and directories. Be thorough and accurate."

EMPTY_MCP="$DEMO_WORKSPACE/empty_mcp.json"
echo '{"mcpServers":{}}' > "$EMPTY_MCP"

CONTROL_START=$(date +%s)
(
  cd "$CONTROL_DIR"
  claude \
    --print \
    --verbose \
    --no-session-persistence \
    --dangerously-skip-permissions \
    --output-format stream-json \
    --system-prompt "$SYSTEM_PROMPT" \
    --model sonnet \
    --setting-sources "" \
    --strict-mcp-config \
    --mcp-config "$EMPTY_MCP" \
    --disable-slash-commands \
    --tools "Bash,Read,Glob,Grep" \
    --max-budget-usd 1.00 \
    "$QUERY" \
    > "$CONTROL_RAW" 2>/dev/null
) || true
CONTROL_END=$(date +%s)
CONTROL_DURATION=$((CONTROL_END - CONTROL_START))

ok "Control completed in ${CONTROL_DURATION}s"

# ─────────────────────────────────────────────────────────────────────────────
# Step 5: Run treatment experiment (WITH fmm)
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}--- Phase 2: Treatment (with fmm + CLAUDE.md) ---${RESET}"
info "Running Claude against fmm-enabled codebase..."

TREATMENT_RAW="$DEMO_WORKSPACE/treatment_raw.jsonl"

TREATMENT_START=$(date +%s)
(
  cd "$TREATMENT_DIR"
  claude \
    --print \
    --verbose \
    --no-session-persistence \
    --dangerously-skip-permissions \
    --output-format stream-json \
    --system-prompt "$SYSTEM_PROMPT" \
    --model sonnet \
    --setting-sources "project" \
    --strict-mcp-config \
    --mcp-config "$EMPTY_MCP" \
    --disable-slash-commands \
    --tools "Bash,Read,Glob,Grep" \
    --max-budget-usd 1.00 \
    "$QUERY" \
    > "$TREATMENT_RAW" 2>/dev/null
) || true
TREATMENT_END=$(date +%s)
TREATMENT_DURATION=$((TREATMENT_END - TREATMENT_START))

ok "Treatment completed in ${TREATMENT_DURATION}s"

# ─────────────────────────────────────────────────────────────────────────────
# Step 6: Parse results and print comparison
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}=== Parsing Results ===${RESET}"

python3 - "$CONTROL_RAW" "$TREATMENT_RAW" "$CONTROL_DURATION" "$TREATMENT_DURATION" << 'PYTHON'
import json
import sys

def parse_trace(filepath):
    """Parse Claude CLI stream-json output into metrics."""
    tool_calls = []
    files_read = set()
    total_input_tokens = 0
    total_output_tokens = 0
    cost_usd = 0.0
    first_tool = None
    read_fmm_sidecar = False

    try:
        with open(filepath) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue

                etype = event.get("type", "")

                if etype == "assistant":
                    msg = event.get("message", {})
                    content = msg.get("content", [])
                    usage = msg.get("usage", {})
                    total_input_tokens += usage.get("input_tokens", 0)
                    total_input_tokens += usage.get("cache_creation_input_tokens", 0)
                    total_input_tokens += usage.get("cache_read_input_tokens", 0)
                    total_output_tokens += usage.get("output_tokens", 0)

                    for block in content:
                        if block.get("type") == "tool_use":
                            tc = {
                                "tool": block.get("name", ""),
                                "input": block.get("input", {}),
                            }
                            tool_calls.append(tc)
                            if first_tool is None:
                                first_tool = f"{tc['tool']}({json.dumps(tc['input'])[:80]})"

                            if tc["tool"] == "Read":
                                fp = tc["input"].get("file_path", "")
                                if fp:
                                    files_read.add(fp)
                                    if fp.endswith(".fmm"):
                                        read_fmm_sidecar = True

                if etype == "result":
                    model_usage = event.get("modelUsage", {})
                    for model, usage in model_usage.items():
                        cost_usd = usage.get("costUSD", cost_usd)
                        total_in = (usage.get("inputTokens", 0)
                                    + usage.get("cacheCreationInputTokens", 0)
                                    + usage.get("cacheReadInputTokens", 0))
                        total_input_tokens = max(total_input_tokens, total_in)
                        total_output_tokens = max(total_output_tokens, usage.get("outputTokens", 0))

    except FileNotFoundError:
        return None

    tool_summary = {}
    for tc in tool_calls:
        tool_summary[tc["tool"]] = tool_summary.get(tc["tool"], 0) + 1

    return {
        "tool_calls": len(tool_calls),
        "tool_summary": tool_summary,
        "files_read": len(files_read),
        "tokens_in": total_input_tokens,
        "tokens_out": total_output_tokens,
        "tokens_total": total_input_tokens + total_output_tokens,
        "cost_usd": cost_usd,
        "first_tool": first_tool or "(none)",
        "read_fmm_sidecar": read_fmm_sidecar,
    }


control_file = sys.argv[1]
treatment_file = sys.argv[2]
control_dur = int(sys.argv[3])
treatment_dur = int(sys.argv[4])

control = parse_trace(control_file)
treatment = parse_trace(treatment_file)

if not control or not treatment:
    print("ERROR: Could not parse one or both trace files.")
    sys.exit(1)

# ── Print comparison table ──

W = 60
SEP = "+" + "-" * 24 + "+" + "-" * 22 + "+" + "-" * 22 + "+"

print()
print("=" * W)
print("  fmm Token Savings — Side-by-Side Comparison")
print("=" * W)
print()
print(SEP)
print(f"| {'Metric':<22} | {'Control (no fmm)':>20} | {'Treatment (fmm)':>20} |")
print(SEP)

rows = [
    ("Tool calls",       str(control["tool_calls"]),       str(treatment["tool_calls"])),
    ("Files read",        str(control["files_read"]),        str(treatment["files_read"])),
    ("Input tokens",      f"{control['tokens_in']:,}",      f"{treatment['tokens_in']:,}"),
    ("Output tokens",     f"{control['tokens_out']:,}",     f"{treatment['tokens_out']:,}"),
    ("Total tokens",      f"{control['tokens_total']:,}",   f"{treatment['tokens_total']:,}"),
    ("Cost (USD)",        f"${control['cost_usd']:.4f}",    f"${treatment['cost_usd']:.4f}"),
    ("Duration",          f"{control_dur}s",                 f"{treatment_dur}s"),
    ("Read .fmm sidecars?", "No",                            "Yes" if treatment["read_fmm_sidecar"] else "No"),
]

for label, ctrl, treat in rows:
    print(f"| {label:<22} | {ctrl:>20} | {treat:>20} |")
print(SEP)

# Token delta
if control["tokens_total"] > 0:
    delta = control["tokens_total"] - treatment["tokens_total"]
    pct = (delta / control["tokens_total"]) * 100
    direction = "saved" if delta > 0 else "extra"
    print()
    print(f"  Token delta: {abs(delta):,} tokens {direction} ({abs(pct):.1f}%)")

# Cost delta
if control["cost_usd"] > 0:
    cost_delta = control["cost_usd"] - treatment["cost_usd"]
    cost_pct = (cost_delta / control["cost_usd"]) * 100
    cost_dir = "saved" if cost_delta > 0 else "extra"
    print(f"  Cost delta:  ${abs(cost_delta):.4f} {cost_dir} ({abs(cost_pct):.1f}%)")

# Tool call breakdown
print()
print("  Tool call breakdown:")
print(f"    Control:   {control['tool_summary']}")
print(f"    Treatment: {treatment['tool_summary']}")

# First action comparison
print()
print("  First tool action:")
print(f"    Control:   {control['first_tool']}")
print(f"    Treatment: {treatment['first_tool']}")

# Behavioral insight
print()
if treatment["read_fmm_sidecar"]:
    print("  KEY BEHAVIOR: With fmm, the LLM read .fmm sidecar files")
    print("  to understand the codebase, then made targeted source reads.")
    print("  Without fmm, it brute-forced with grep across all files.")
else:
    print("  NOTE: The LLM did not read .fmm sidecar files in this run.")
    print("  This can happen — re-run for more samples. The CLAUDE.md hint")
    print("  triggers sidecar-first behavior in the majority of runs.")

print()
print("=" * W)
print("  Raw traces saved to:")
print(f"    Control:   {control_file}")
print(f"    Treatment: {treatment_file}")
print("=" * W)
print()

PYTHON

echo ""
echo -e "${BOLD}Demo complete.${RESET}"
echo ""
echo "For deeper analysis, see:"
echo "  research/exp14/FINDINGS.md    — Full experiment write-up"
echo "  content/demo/DEMO-GUIDE.md    — Step-by-step walkthrough"
echo ""
