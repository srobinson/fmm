#!/usr/bin/env python3
"""Score exp17 CLI results: correctness, tool calls, tokens, cost."""

import json
import os
import glob
import re


def parse_stream_json(filepath):
    """Extract metrics from Claude CLI stream-json output."""
    tool_calls = []
    files_read = set()
    final_text = ""
    total_cost = 0.0
    total_input = 0
    total_output = 0
    total_cache_read = 0
    total_cache_create = 0
    num_turns = 0
    duration_ms = 0
    mcp_servers = []
    permission_denials = []

    for line in open(filepath, encoding="utf-8", errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        msg_type = msg.get("type", "")

        # Init message — MCP servers
        if msg_type == "system" and msg.get("subtype") == "init":
            mcp_servers = [s.get("name", "") for s in msg.get("mcp_servers", [])]

        # Assistant messages — tool calls and text
        if msg_type == "assistant":
            for block in msg.get("message", {}).get("content", []):
                if isinstance(block, dict):
                    if block.get("type") == "tool_use":
                        name = block.get("name", "")
                        tool_calls.append(name)
                        inp = block.get("input", {})
                        if name == "Read" and "file_path" in inp:
                            files_read.add(inp["file_path"])
                        if name == "Grep" and "path" in inp:
                            files_read.add(inp["path"])
                    elif block.get("type") == "text":
                        final_text = block.get("text", "")

            usage = msg.get("message", {}).get("usage", {})
            total_input += usage.get("input_tokens", 0)
            total_output += usage.get("output_tokens", 0)
            total_cache_read += usage.get("cache_read_input_tokens", 0)
            total_cache_create += usage.get("cache_creation_input_tokens", 0)

        # Result message — final answer + aggregate metrics
        if msg_type == "result":
            final_text = msg.get("result", final_text) or final_text
            total_cost = msg.get("total_cost_usd", 0.0)
            num_turns = msg.get("num_turns", 0)
            duration_ms = msg.get("duration_ms", 0)
            permission_denials = msg.get("permission_denials", [])

            # Use result-level usage if available (more accurate)
            rusage = msg.get("usage", {})
            if rusage.get("input_tokens"):
                total_input = rusage["input_tokens"]
            if rusage.get("output_tokens"):
                total_output = rusage["output_tokens"]
            if rusage.get("cache_read_input_tokens"):
                total_cache_read = rusage["cache_read_input_tokens"]
            if rusage.get("cache_creation_input_tokens"):
                total_cache_create = rusage["cache_creation_input_tokens"]

    return {
        "tool_calls": tool_calls,
        "tool_call_count": len(tool_calls),
        "fmm_calls": [t for t in tool_calls if "fmm" in t.lower()],
        "grep_calls": sum(1 for t in tool_calls if t == "Grep"),
        "glob_calls": sum(1 for t in tool_calls if t == "Glob"),
        "read_calls": sum(1 for t in tool_calls if t == "Read"),
        "bash_calls": sum(1 for t in tool_calls if t == "Bash"),
        "input_tokens": total_input,
        "output_tokens": total_output,
        "cache_read_tokens": total_cache_read,
        "cache_create_tokens": total_cache_create,
        "total_tokens": total_input + total_output + total_cache_read + total_cache_create,
        "cost_usd": total_cost,
        "num_turns": num_turns,
        "duration_ms": duration_ms,
        "files_read": len(files_read),
        "final_text": final_text or "",
        "mcp_servers": mcp_servers,
        "permission_denials": len(permission_denials),
    }


def score_exact_path(answer, truth):
    answer = answer.strip().replace("./", "").strip("`").strip()
    truth = truth.strip().replace("./", "")
    return 1.0 if truth in answer else 0.0


def score_set_match(answer, truth_list):
    if not truth_list:
        return 1.0
    answer_lower = answer.lower().strip()
    found = 0
    for item in truth_list:
        stem = item.split("/")[-1].replace(".js", "").replace(".ts", "")
        if stem.lower() in answer_lower:
            found += 1
    return found / len(truth_list)


def score_exact_number(answer, truth):
    numbers = re.findall(r"\b(\d+)\b", answer)
    return 1.0 if truth in numbers else 0.0


def score_result(answer, task):
    scoring = task["scoring"]
    truth = task["ground_truth"]
    if scoring == "exact_path":
        return score_exact_path(answer, truth)
    elif scoring == "set_match":
        return score_set_match(answer, truth)
    elif scoring == "exact_number":
        return score_exact_number(answer, str(truth))
    return 0.0


def main():
    tasks = json.load(open("tasks.json"))["tasks"]
    task_map = {t["id"]: t for t in tasks}

    conditions = ["A", "B"]
    results = {c: {} for c in conditions}

    for cond in conditions:
        for filepath in sorted(glob.glob(f"results/{cond}/*.jsonl")):
            basename = os.path.basename(filepath).replace(".jsonl", "")
            parts = basename.rsplit("_run", 1)
            task_id = parts[0]
            run_num = parts[1] if len(parts) > 1 else "1"

            if task_id not in task_map:
                continue

            metrics = parse_stream_json(filepath)
            correctness = score_result(metrics["final_text"], task_map[task_id])

            key = f"{task_id}_run{run_num}"
            results[cond][key] = {
                **metrics,
                "task_id": task_id,
                "run": run_num,
                "correctness": correctness,
            }

    # ── Per-task detail ──
    print("=" * 90)
    print("exp17: CLI A/B Cost Comparison")
    print("=" * 90)
    print()

    for task in tasks:
        tid = task["id"]
        print(f"--- {tid} ---")
        print(f"  Prompt: {task['prompt'][:80]}...")
        print()

        for cond in conditions:
            runs = {k: v for k, v in results[cond].items() if v["task_id"] == tid}
            if not runs:
                print(f"  [{cond}] No results")
                continue

            for key, r in sorted(runs.items()):
                status = "PASS" if r["correctness"] >= 0.8 else "FAIL"
                fmm_str = f", fmm={len(r['fmm_calls'])}" if r["fmm_calls"] else ""
                denied = f", denied={r['permission_denials']}" if r["permission_denials"] else ""
                print(
                    f"  [{cond}] {status} ({r['correctness']:.0%}) | "
                    f"tools={r['tool_call_count']} (grep={r['grep_calls']}, "
                    f"read={r['read_calls']}{fmm_str}{denied}) | "
                    f"turns={r['num_turns']} | "
                    f"tokens={r['total_tokens']:,} | "
                    f"cost=${r['cost_usd']:.4f} | "
                    f"{r['duration_ms']/1000:.1f}s"
                )
                if r["correctness"] < 0.8:
                    preview = r["final_text"][:120].replace("\n", " ")
                    print(f"       Answer: {preview}")
        print()

    # ── Aggregate ──
    print("=" * 90)
    print("AGGREGATE COMPARISON")
    print("=" * 90)
    print()

    a_runs = list(results["A"].values())
    b_runs = list(results["B"].values())

    if not a_runs or not b_runs:
        print("Missing results for one or both conditions.")
        return

    header = f"{'Metric':<25} {'A (vanilla)':<20} {'B (fmm)':<20} {'Δ':<15}"
    print(header)
    print("-" * 80)

    metrics = [
        ("Correctness", "correctness", lambda x: f"{x:.0%}"),
        ("Turns (avg)", "num_turns", lambda x: f"{x:.1f}"),
        ("Tool calls (avg)", "tool_call_count", lambda x: f"{x:.1f}"),
        ("Grep calls (avg)", "grep_calls", lambda x: f"{x:.1f}"),
        ("Read calls (avg)", "read_calls", lambda x: f"{x:.1f}"),
        ("Files read (avg)", "files_read", lambda x: f"{x:.1f}"),
        ("Total tokens (avg)", "total_tokens", lambda x: f"{x:,.0f}"),
        ("Output tokens (avg)", "output_tokens", lambda x: f"{x:,.0f}"),
        ("Duration (avg, s)", "duration_ms", lambda x: f"{x/1000:.1f}"),
    ]

    for label, key, fmt in metrics:
        a_val = sum(r[key] for r in a_runs) / len(a_runs)
        b_val = sum(r[key] for r in b_runs) / len(b_runs)
        if a_val > 0:
            delta = ((b_val - a_val) / a_val) * 100
            delta_str = f"{delta:+.0f}%"
        else:
            delta_str = "n/a"
        print(f"{label:<25} {fmt(a_val):<20} {fmt(b_val):<20} {delta_str:<15}")

    # fmm adoption
    b_fmm_count = sum(len(r["fmm_calls"]) for r in b_runs)
    b_total_tools = sum(r["tool_call_count"] for r in b_runs)
    fmm_pct = (b_fmm_count / b_total_tools * 100) if b_total_tools else 0
    print(f"{'fmm tool calls (B)':<25} {'n/a':<20} {b_fmm_count:<20} {fmm_pct:.0f}% of tools")

    # Cost (actual from CLI)
    a_cost = sum(r["cost_usd"] for r in a_runs)
    b_cost = sum(r["cost_usd"] for r in b_runs)
    if a_cost > 0:
        cost_delta = ((b_cost - a_cost) / a_cost) * 100
        print(f"\n{'Total cost':<25} ${a_cost:<19.4f} ${b_cost:<19.4f} {cost_delta:+.0f}%")
        print(f"{'Per-task cost':<25} ${a_cost/len(a_runs):<19.4f} ${b_cost/len(b_runs):<19.4f} {cost_delta:+.0f}%")
    else:
        print(f"\n{'Total cost':<25} ${a_cost:<19.4f} ${b_cost:<19.4f}")

    # Dump raw
    json.dump(results, open("results/scored.json", "w"), indent=2, default=str)
    print()
    print(f"Raw data: results/scored.json")


if __name__ == "__main__":
    main()
