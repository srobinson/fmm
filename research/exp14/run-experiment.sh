#!/usr/bin/env bash
set -euo pipefail

# run-experiment.sh — Run isolated Claude CLI experiment against a codebase variant
#
# Usage: ./run-experiment.sh <variant> <task> [run_id]
#   variant: clean | inline | manifest | hint
#   task: The prompt to give Claude
#   run_id: Optional run identifier (default: 1)
#
# Output: JSON trace saved to results/<category>/<variant>_run<N>.json

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VARIANT="${1:?Usage: $0 <variant> <task> [run_id]}"
TASK="${2:?Usage: $0 <variant> <task> [run_id]}"
RUN_ID="${3:-1}"

# Determine repo directory and result category
case "$VARIANT" in
  clean|inline|manifest)
    REPO_DIR="$SCRIPT_DIR/repos/$VARIANT"
    RESULT_DIR="$SCRIPT_DIR/results/baseline"
    ;;
  hint)
    REPO_DIR="$SCRIPT_DIR/repos/hint"
    RESULT_DIR="$SCRIPT_DIR/results/hint"
    ;;
  *)
    echo "Unknown variant: $VARIANT" >&2
    exit 1
    ;;
esac

if [ ! -d "$REPO_DIR" ]; then
  echo "Repo directory not found: $REPO_DIR" >&2
  exit 1
fi

mkdir -p "$RESULT_DIR"

OUTPUT_FILE="$RESULT_DIR/${VARIANT}_run${RUN_ID}.json"
RAW_FILE="$RESULT_DIR/${VARIANT}_run${RUN_ID}_raw.jsonl"

echo "=== Experiment: variant=$VARIANT run=$RUN_ID ==="
echo "Repo: $REPO_DIR"
echo "Task: $TASK"
echo "Output: $OUTPUT_FILE"

START_TIME=$(date +%s)

SYSTEM_PROMPT="You are a helpful coding assistant. You help developers navigate and understand codebases. Use available tools to explore files and directories. Be thorough and accurate."

# For hint variant, don't delete .claude/CLAUDE.md — that's the hint!
if [ "$VARIANT" != "hint" ]; then
  rm -rf "$REPO_DIR/.claude" "$REPO_DIR/CLAUDE.md" 2>/dev/null || true
fi

# Create empty MCP config to prevent any MCP server loading
EMPTY_MCP=$(mktemp)
echo '{"mcpServers":{}}' > "$EMPTY_MCP"

# Build extra args for hint variant
EXTRA_ARGS=()
if [ "$VARIANT" = "hint" ]; then
  EXTRA_ARGS+=(--append-system-prompt "Check .fmm/ for codebase index")
fi

# Run Claude from within the variant directory for proper cwd isolation
# Use subshell to avoid changing parent shell cwd
# --setting-sources "": skip user/project/local settings (prevents CLAUDE.md loading)
# --strict-mcp-config + empty config: no MCP servers (prevents fmm knowledge leakage)
(
  cd "$REPO_DIR"
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
    --tools "Bash,Read,Glob,Grep,Write,Edit" \
    --max-budget-usd 1.00 \
    "${EXTRA_ARGS[@]}" \
    "$TASK" \
    > "$RAW_FILE" 2>/dev/null
) || true

rm -f "$EMPTY_MCP"

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo "Completed in ${DURATION}s"

# Parse the stream-json output to extract metrics
python3 - "$RAW_FILE" "$OUTPUT_FILE" "$VARIANT" "$RUN_ID" "$DURATION" "$TASK" << 'PYTHON'
import json
import sys

raw_file = sys.argv[1]
output_file = sys.argv[2]
variant = sys.argv[3]
run_id = sys.argv[4]
duration = int(sys.argv[5])
task = sys.argv[6]

tool_calls = []
files_read = []
assistant_text = []
total_input_tokens = 0
total_output_tokens = 0
total_cost = 0.0

with open(raw_file) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue

        etype = event.get("type", "")

        # Claude CLI stream-json format uses "assistant" messages with full content
        if etype == "assistant":
            msg = event.get("message", {})
            content = msg.get("content", [])
            usage = msg.get("usage", {})
            total_input_tokens += usage.get("input_tokens", 0)
            total_input_tokens += usage.get("cache_creation_input_tokens", 0)
            total_input_tokens += usage.get("cache_read_input_tokens", 0)
            total_output_tokens += usage.get("output_tokens", 0)

            for block in content:
                if block.get("type") == "text":
                    assistant_text.append(block.get("text", ""))
                elif block.get("type") == "tool_use":
                    tc = {
                        "tool": block.get("name", ""),
                        "id": block.get("id", ""),
                        "input": block.get("input", {}),
                    }
                    tool_calls.append(tc)

                    # Track file reads
                    tool_name = tc["tool"]
                    tool_input = tc["input"]
                    if tool_name == "Read":
                        fp = tool_input.get("file_path", "")
                        if fp:
                            files_read.append(fp)
                    elif tool_name == "Bash":
                        cmd = tool_input.get("command", "")
                        if any(x in cmd for x in ["cat ", "head ", "tail ", "less "]):
                            files_read.append(cmd)

        # Result event has total usage
        if etype == "result":
            model_usage = event.get("modelUsage", {})
            for model, usage in model_usage.items():
                total_cost = usage.get("costUSD", total_cost)
                total_input_tokens = max(total_input_tokens, usage.get("inputTokens", 0) + usage.get("cacheCreationInputTokens", 0) + usage.get("cacheReadInputTokens", 0))
                total_output_tokens = max(total_output_tokens, usage.get("outputTokens", 0))

full_text = "\n".join(assistant_text)

# Detect fmm discovery — did the LLM interact with .fmm/ in any way?
discovered_fmm = any(
    ".fmm" in json.dumps(tc.get("input", {}))
    for tc in tool_calls
) or ".fmm" in full_text

# Detect manifest usage — did it specifically read .fmm/index.json?
used_manifest = any(
    "index.json" in json.dumps(tc.get("input", {})) and ".fmm" in json.dumps(tc.get("input", {}))
    for tc in tool_calls
)

# Check if FMM inline comments were noticed
noticed_inline = "FMM" in full_text or "frontmatter" in full_text.lower()

# Count unique files read
unique_files = list(set(files_read))

# Build result
result = {
    "variant": variant,
    "run_id": run_id,
    "task": task,
    "duration_seconds": duration,
    "tool_calls_count": len(tool_calls),
    "tool_calls_summary": {},
    "tool_calls": [{"tool": tc["tool"], "input": tc["input"]} for tc in tool_calls],
    "files_read": unique_files,
    "files_read_count": len(unique_files),
    "total_tool_calls_by_type": {},
    "tokens_in": total_input_tokens,
    "tokens_out": total_output_tokens,
    "tokens_total": total_input_tokens + total_output_tokens,
    "cost_usd": total_cost,
    "discovered_fmm": discovered_fmm,
    "used_manifest": used_manifest,
    "noticed_inline_comments": noticed_inline,
    "assistant_response": full_text[:8000],
}

# Summarize tool calls by type
for tc in tool_calls:
    name = tc["tool"]
    result["total_tool_calls_by_type"][name] = result["total_tool_calls_by_type"].get(name, 0) + 1

with open(output_file, "w") as f:
    json.dump(result, f, indent=2)

print(f"  Tool calls: {len(tool_calls)}")
print(f"  Files read: {len(unique_files)}")
print(f"  Tokens: {total_input_tokens} in / {total_output_tokens} out")
print(f"  Cost: ${total_cost:.4f}")
print(f"  Discovered FMM: {discovered_fmm}")
print(f"  Used manifest: {used_manifest}")
print(f"  Noticed inline: {noticed_inline}")
PYTHON

echo "Trace saved to $OUTPUT_FILE"
