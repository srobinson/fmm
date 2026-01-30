#!/usr/bin/env python3
"""Score exp16 results: correctness, tool calls, tokens, files read."""

import json
import os
import glob
import re
import sys


def parse_jsonl(filepath):
    """Extract metrics from a Claude JSONL output file."""
    tool_calls = []
    total_input_tokens = 0
    total_output_tokens = 0
    total_cache_read = 0
    total_cache_create = 0
    files_read = set()
    final_text = ""
    mcp_servers = []
    skills = []

    for line in open(filepath, encoding="utf-8", errors="replace"):
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        # Init message
        if msg.get("type") == "system" and msg.get("subtype") == "init":
            mcp_servers = [s.get("name", "") for s in msg.get("mcp_servers", [])]
            skills = msg.get("skills", [])

        # Assistant messages
        if msg.get("type") == "assistant":
            usage = msg.get("message", {}).get("usage", {})
            total_input_tokens += usage.get("input_tokens", 0)
            total_output_tokens += usage.get("output_tokens", 0)
            total_cache_read += usage.get("cache_read_input_tokens", 0)
            total_cache_create += usage.get("cache_creation_input_tokens", 0)

            for block in msg.get("message", {}).get("content", []):
                if isinstance(block, dict):
                    if block.get("type") == "tool_use":
                        name = block.get("name", "")
                        tool_calls.append(name)
                        # Track files read
                        inp = block.get("input", {})
                        if name == "Read" and "file_path" in inp:
                            files_read.add(inp["file_path"])
                        if name == "Grep" and "path" in inp:
                            files_read.add(inp["path"])
                    elif block.get("type") == "text":
                        final_text = block.get("text", "")

        # Result messages (for final text output)
        if msg.get("type") == "result":
            final_text = msg.get("result", final_text)

    return {
        "tool_calls": tool_calls,
        "tool_call_count": len(tool_calls),
        "fmm_calls": [t for t in tool_calls if "fmm" in t.lower()],
        "grep_calls": sum(1 for t in tool_calls if t == "Grep"),
        "glob_calls": sum(1 for t in tool_calls if t == "Glob"),
        "read_calls": sum(1 for t in tool_calls if t == "Read"),
        "bash_calls": sum(1 for t in tool_calls if t == "Bash"),
        "input_tokens": total_input_tokens,
        "output_tokens": total_output_tokens,
        "cache_read_tokens": total_cache_read,
        "cache_create_tokens": total_cache_create,
        "total_tokens": total_input_tokens + total_output_tokens + total_cache_read + total_cache_create,
        "files_read": len(files_read),
        "final_text": final_text,
        "mcp_servers": mcp_servers,
        "skills": skills,
    }


def score_exact_path(answer, truth):
    """Score: does the answer contain the exact file path?"""
    # Normalize: strip leading ./ and whitespace
    answer = answer.strip().replace("./", "")
    truth = truth.strip().replace("./", "")
    return 1.0 if truth in answer else 0.0


def score_set_match(answer, truth_list):
    """Score: what fraction of ground truth items appear in the answer?"""
    if not truth_list:
        return 1.0
    answer_lower = answer.lower().strip()
    found = 0
    for item in truth_list:
        # Match on filename stem (without extension, without path prefix)
        stem = item.split("/")[-1].replace(".js", "").replace(".ts", "")
        if stem.lower() in answer_lower:
            found += 1
    return found / len(truth_list)


def score_exact_number(answer, truth):
    """Score: does the answer contain the exact number?"""
    numbers = re.findall(r"\b(\d+)\b", answer)
    return 1.0 if truth in numbers else 0.0


def score_result(answer, task):
    """Score an answer against ground truth."""
    scoring = task["scoring"]
    truth = task["ground_truth"]

    if scoring == "exact_path":
        return score_exact_path(answer, truth)
    elif scoring == "set_match":
        return score_set_match(answer, truth)
    elif scoring == "exact_number":
        return score_exact_number(answer, str(truth))
    else:
        return 0.0


def main():
    tasks = json.load(open("tasks.json"))["tasks"]
    task_map = {t["id"]: t for t in tasks}

    conditions = ["A", "B"]
    results = {c: {} for c in conditions}

    for cond in conditions:
        for filepath in sorted(glob.glob(f"results/{cond}/*.jsonl")):
            basename = os.path.basename(filepath).replace(".jsonl", "")
            # Parse task_id from filename: e.g., "symbol-lookup-1_run1"
            parts = basename.rsplit("_run", 1)
            task_id = parts[0]
            run_num = parts[1] if len(parts) > 1 else "1"

            if task_id not in task_map:
                continue

            metrics = parse_jsonl(filepath)
            correctness = score_result(metrics["final_text"], task_map[task_id])

            key = f"{task_id}_run{run_num}"
            results[cond][key] = {
                **metrics,
                "task_id": task_id,
                "run": run_num,
                "correctness": correctness,
            }

    # Print comparison
    print("=" * 80)
    print("exp16: A/B Cost Comparison")
    print("=" * 80)
    print()

    # Per-task comparison
    for task in tasks:
        tid = task["id"]
        print(f"--- {tid} ---")
        print(f"  Prompt: {task['prompt'][:80]}...")
        print(f"  Truth:  {task['ground_truth']}")
        print()

        for cond in conditions:
            runs = {k: v for k, v in results[cond].items() if v["task_id"] == tid}
            if not runs:
                print(f"  [{cond}] No results")
                continue

            for key, r in sorted(runs.items()):
                correct_str = f"{'PASS' if r['correctness'] >= 0.8 else 'FAIL'} ({r['correctness']:.0%})"
                fmm_str = f", fmm={len(r['fmm_calls'])}" if r["fmm_calls"] else ""
                print(
                    f"  [{cond}] {correct_str} | "
                    f"tools={r['tool_call_count']} (grep={r['grep_calls']}, "
                    f"glob={r['glob_calls']}, read={r['read_calls']}, "
                    f"bash={r['bash_calls']}{fmm_str}) | "
                    f"tokens={r['total_tokens']:,} | "
                    f"files={r['files_read']}"
                )
                if r["correctness"] < 0.8:
                    # Show what Claude answered
                    answer_preview = r["final_text"][:120].replace("\n", " ")
                    print(f"       Answer: {answer_preview}")
        print()

    # Aggregate comparison
    print("=" * 80)
    print("AGGREGATE COMPARISON")
    print("=" * 80)
    print()
    print(f"{'Metric':<25} {'A (vanilla)':<20} {'B (fmm)':<20} {'Δ':<15}")
    print("-" * 80)

    for cond in conditions:
        all_runs = list(results[cond].values())
        if not all_runs:
            continue

    a_runs = list(results["A"].values())
    b_runs = list(results["B"].values())

    if a_runs and b_runs:
        metrics = [
            ("Correctness", "correctness", lambda x: f"{x:.0%}"),
            ("Tool calls (avg)", "tool_call_count", lambda x: f"{x:.1f}"),
            ("Grep calls (avg)", "grep_calls", lambda x: f"{x:.1f}"),
            ("Glob calls (avg)", "glob_calls", lambda x: f"{x:.1f}"),
            ("Read calls (avg)", "read_calls", lambda x: f"{x:.1f}"),
            ("Bash calls (avg)", "bash_calls", lambda x: f"{x:.1f}"),
            ("Files read (avg)", "files_read", lambda x: f"{x:.1f}"),
            ("Total tokens (avg)", "total_tokens", lambda x: f"{x:,.0f}"),
            ("Input tokens (avg)", "input_tokens", lambda x: f"{x:,.0f}"),
            ("Output tokens (avg)", "output_tokens", lambda x: f"{x:,.0f}"),
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

        print()

        # Cost estimate (Sonnet pricing: $3/MTok input, $15/MTok output, $0.30/MTok cache read)
        a_cost = sum(
            r["input_tokens"] * 3 / 1_000_000
            + r["output_tokens"] * 15 / 1_000_000
            + r["cache_read_tokens"] * 0.30 / 1_000_000
            + r["cache_create_tokens"] * 3.75 / 1_000_000
            for r in a_runs
        )
        b_cost = sum(
            r["input_tokens"] * 3 / 1_000_000
            + r["output_tokens"] * 15 / 1_000_000
            + r["cache_read_tokens"] * 0.30 / 1_000_000
            + r["cache_create_tokens"] * 3.75 / 1_000_000
            for r in b_runs
        )
        print(f"{'Est. cost (all tasks)':<25} ${a_cost:<19.4f} ${b_cost:<19.4f} {((b_cost-a_cost)/a_cost)*100:+.0f}%")

    # Dump raw data
    json.dump(results, open("results/scored.json", "w"), indent=2, default=str)
    print()
    print("Raw data: results/scored.json")


if __name__ == "__main__":
    main()
