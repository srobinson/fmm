#!/usr/bin/env python3
"""Parse Claude CLI stream-json output into structured metrics and transcript."""
import json
import sys

raw_file = sys.argv[1]
result_file = sys.argv[2]
transcript_file = sys.argv[3]
condition = sys.argv[4]
label = sys.argv[5]
duration = int(sys.argv[6])
query = sys.argv[7]

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

                    tool_name = tc["tool"]
                    tool_input = tc["input"]
                    if tool_name == "Read":
                        fp = tool_input.get("file_path", "")
                        if fp:
                            files_read.append(fp)
                    elif tool_name == "Bash":
                        cmd = tool_input.get("command", "")
                        if any(x in cmd for x in ["cat ", "head ", "tail ", "less "]):
                            files_read.append(f"(bash) {cmd[:100]}")

        if etype == "result":
            model_usage = event.get("modelUsage", {})
            for _model, usage in model_usage.items():
                total_cost = usage.get("costUSD", total_cost)
                total_input_tokens = max(
                    total_input_tokens,
                    usage.get("inputTokens", 0)
                    + usage.get("cacheCreationInputTokens", 0)
                    + usage.get("cacheReadInputTokens", 0),
                )
                total_output_tokens = max(
                    total_output_tokens, usage.get("outputTokens", 0)
                )

full_text = "\n".join(assistant_text)
unique_files = sorted(set(files_read))

# Detect fmm interaction
discovered_fmm = any(
    ".fmm" in json.dumps(tc.get("input", {})) for tc in tool_calls
) or ".fmm" in full_text

used_manifest = any(
    "index.json" in json.dumps(tc.get("input", {}))
    and ".fmm" in json.dumps(tc.get("input", {}))
    for tc in tool_calls
)

# Count tool calls by type
tool_counts = {}
for tc in tool_calls:
    name = tc["tool"]
    tool_counts[name] = tool_counts.get(name, 0) + 1

# Build result
result = {
    "condition": condition,
    "label": label,
    "query": query,
    "duration_seconds": duration,
    "tool_calls_count": len(tool_calls),
    "tool_calls_by_type": tool_counts,
    "tool_calls": [{"tool": tc["tool"], "input": tc["input"]} for tc in tool_calls],
    "files_read": unique_files,
    "files_read_count": len(unique_files),
    "tokens_in": total_input_tokens,
    "tokens_out": total_output_tokens,
    "tokens_total": total_input_tokens + total_output_tokens,
    "cost_usd": round(total_cost, 4),
    "discovered_fmm": discovered_fmm,
    "used_manifest": used_manifest,
    "response_lines": len(full_text.splitlines()),
}

with open(result_file, "w") as f:
    json.dump(result, f, indent=2)

# Write human-readable transcript
with open(transcript_file, "w") as f:
    f.write(f"# {condition.upper()} — {label}\n")
    f.write(f"# Query: {query}\n")
    f.write(f"# Duration: {duration}s\n")
    f.write(f"# Tool calls: {len(tool_calls)}\n")
    f.write(f"# Files read: {len(unique_files)}\n")
    f.write(f"# Tokens: {total_input_tokens} in / {total_output_tokens} out\n")
    f.write(f"# Cost: ${total_cost:.4f}\n")
    f.write(f"# FMM discovered: {discovered_fmm}\n")
    f.write(f"# Manifest used: {used_manifest}\n")
    f.write("\n")

    f.write("## Tool Call Sequence\n\n")
    for i, tc in enumerate(tool_calls, 1):
        tool_name = tc["tool"]
        inp = tc["input"]
        if tool_name == "Read":
            f.write(f"  {i}. Read({inp.get('file_path', '?')})\n")
        elif tool_name == "Glob":
            f.write(f"  {i}. Glob({inp.get('pattern', '?')})\n")
        elif tool_name == "Grep":
            f.write(f"  {i}. Grep({inp.get('pattern', '?')})\n")
        elif tool_name == "Bash":
            f.write(f"  {i}. Bash({inp.get('command', '?')[:80]})\n")
        else:
            f.write(f"  {i}. {tool_name}({json.dumps(inp)[:80]})\n")

    f.write("\n## LLM Response\n\n")
    f.write(full_text[:10000])
    if len(full_text) > 10000:
        f.write("\n\n... (truncated)")

# Print summary to stdout
print(f"  Tool calls: {len(tool_calls)} ({', '.join(f'{k}:{v}' for k, v in tool_counts.items())})")
print(f"  Files read: {len(unique_files)}")
print(f"  Tokens: {total_input_tokens} in / {total_output_tokens} out ({total_input_tokens + total_output_tokens} total)")
print(f"  Cost: ${total_cost:.4f}")
print(f"  FMM discovered: {discovered_fmm} | Manifest used: {used_manifest}")
