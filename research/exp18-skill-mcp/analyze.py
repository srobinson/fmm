#!/usr/bin/env python3
"""Analyze Exp18 results from stream-json output files."""

import json
import os
import sys
from pathlib import Path
from collections import defaultdict


def parse_stream_json(filepath):
    """Parse a stream-json file and extract metrics."""
    metrics = {
        "tool_calls": 0,
        "tools_by_name": defaultdict(int),
        "files_read": [],
        "sidecar_reads": 0,
        "source_reads": 0,
        "read_calls": 0,
        "input_tokens": 0,
        "output_tokens": 0,
        "cache_read_tokens": 0,
        "total_cost_usd": 0.0,
        "num_turns": 0,
        "duration_ms": 0,
        "mcp_calls": 0,
        "response": "",
    }

    with open(filepath) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                data = json.loads(line)
            except json.JSONDecodeError:
                continue

            msg_type = data.get("type")

            if msg_type == "assistant":
                message = data.get("message", {})
                content = message.get("content", [])
                for item in content:
                    if item.get("type") == "tool_use":
                        metrics["tool_calls"] += 1
                        name = item.get("name", "unknown")
                        metrics["tools_by_name"][name] += 1

                        if name.startswith("mcp__fmm__") or name.startswith("fmm_"):
                            metrics["mcp_calls"] += 1

                        if name in ("Read", "View"):
                            metrics["read_calls"] += 1
                            inp = item.get("input", {})
                            path = inp.get("file_path") or inp.get("path", "")
                            if path:
                                metrics["files_read"].append(path)
                                if path.endswith(".fmm"):
                                    metrics["sidecar_reads"] += 1
                                else:
                                    metrics["source_reads"] += 1

                    elif item.get("type") == "text":
                        metrics["response"] = item.get("text", "")

            elif msg_type == "result":
                usage = data.get("usage", {})
                metrics["input_tokens"] = usage.get("input_tokens", 0)
                metrics["output_tokens"] = usage.get("output_tokens", 0)
                metrics["cache_read_tokens"] = usage.get("cache_read_input_tokens", 0)
                metrics["total_cost_usd"] = data.get("total_cost_usd", 0.0)
                metrics["num_turns"] = data.get("num_turns", 0)
                metrics["duration_ms"] = data.get("duration_ms", 0)

                if not metrics["response"]:
                    metrics["response"] = data.get("result", "")

    return dict(metrics)


def classify_reads(files_read, response):
    """Classify reads as exploration, pre-edit, or reference.

    Simple heuristic: if the response mentions editing/modifying a file, that read
    was pre-edit. Otherwise it's exploration.
    """
    edited_files = set()
    # Look for common edit indicators in the response
    for path in files_read:
        basename = os.path.basename(path)
        if any(
            indicator in response.lower()
            for indicator in [
                f"edit {basename}".lower(),
                f"modify {basename}".lower(),
                f"update {basename}".lower(),
                f"write {basename}".lower(),
            ]
        ):
            edited_files.add(path)

    exploration = [f for f in files_read if f not in edited_files and not f.endswith(".fmm")]
    pre_edit = [f for f in files_read if f in edited_files]
    sidecar = [f for f in files_read if f.endswith(".fmm")]

    return {
        "exploration": exploration,
        "pre_edit": pre_edit,
        "sidecar": sidecar,
    }


def main():
    script_dir = Path(__file__).parent
    results_dir = script_dir / "results"

    if not results_dir.exists():
        print(f"No results directory found at {results_dir}")
        sys.exit(1)

    conditions = {"A": [], "B": []}

    for cond in ["A", "B"]:
        cond_dir = results_dir / cond
        if not cond_dir.exists():
            continue
        for f in sorted(cond_dir.glob("*.jsonl")):
            metrics = parse_stream_json(f)
            metrics["file"] = f.name
            conditions[cond].append(metrics)

    if not conditions["A"] and not conditions["B"]:
        print("No results found.")
        sys.exit(1)

    # Print per-run details
    for cond_name, label in [("A", "Control"), ("B", "Sidecar + Skill + MCP")]:
        runs = conditions[cond_name]
        if not runs:
            continue

        print(f"\n{'=' * 60}")
        print(f"Condition {cond_name}: {label}")
        print(f"{'=' * 60}")

        for i, run in enumerate(runs):
            print(f"\n  Run {i+1}: {run['file']}")
            print(f"    Tool calls:    {run['tool_calls']}")
            print(f"    Read calls:    {run['read_calls']}")
            print(f"    Source reads:  {run['source_reads']}")
            print(f"    Sidecar reads: {run['sidecar_reads']}")
            print(f"    MCP calls:     {run['mcp_calls']}")
            print(f"    Cost:          ${run['total_cost_usd']:.4f}")
            print(f"    Turns:         {run['num_turns']}")
            print(f"    Duration:      {run['duration_ms']}ms")
            print(f"    Tools: {dict(run['tools_by_name'])}")

    # Print comparison summary
    if conditions["A"] and conditions["B"]:
        print(f"\n{'=' * 60}")
        print("COMPARISON SUMMARY")
        print(f"{'=' * 60}")

        for metric in ["tool_calls", "read_calls", "source_reads", "total_cost_usd", "num_turns"]:
            a_avg = sum(r[metric] for r in conditions["A"]) / len(conditions["A"])
            b_avg = sum(r[metric] for r in conditions["B"]) / len(conditions["B"])
            if a_avg > 0:
                delta = ((a_avg - b_avg) / a_avg) * 100
                print(f"  {metric:20s}: A={a_avg:.1f}, B={b_avg:.1f}, delta={delta:+.1f}%")
            else:
                print(f"  {metric:20s}: A={a_avg:.1f}, B={b_avg:.1f}")

        # MCP adoption
        mcp_runs = sum(1 for r in conditions["B"] if r["mcp_calls"] > 0)
        total_b = len(conditions["B"])
        print(f"\n  MCP adoption: {mcp_runs}/{total_b} runs used fmm MCP tools")

        # Sidecar reads
        sidecar_total = sum(r["sidecar_reads"] for r in conditions["B"])
        print(f"  Sidecar reads (total): {sidecar_total}")


if __name__ == "__main__":
    main()
