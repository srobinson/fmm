#!/usr/bin/env python3
"""Parse exp15 stream-json results into a comparison table."""

import json
import sys
from pathlib import Path
from collections import defaultdict

RESULTS_DIR = Path(__file__).parent / "results"
CONDITIONS = ["A", "B", "C", "D"]
COND_LABELS = {
    "A": "CLAUDE.md only",
    "B": "Skill only",
    "C": "MCP only",
    "D": "Skill + MCP",
}


def parse_run(filepath: Path) -> dict:
    """Parse a single run's stream-json output (verbose format)."""
    metrics = {
        "tool_calls": 0,
        "read_calls": 0,
        "glob_calls": 0,
        "grep_calls": 0,
        "bash_calls": 0,
        "mcp_calls": 0,
        "fmm_cli_calls": 0,
        "files_accessed": set(),
        "manifest_accessed": False,
        "input_tokens": 0,
        "output_tokens": 0,
        "cache_read_tokens": 0,
        "cache_creation_tokens": 0,
        "duration_ms": 0,
        "num_turns": 0,
        "cost_usd": 0.0,
    }

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

            # Our appended metadata
            if "_meta" in event:
                metrics["duration_ms"] = event["_meta"].get("duration_ms", 0)
                continue

            # Final result event — has aggregate stats
            if etype == "result":
                metrics["duration_ms"] = event.get("duration_ms", metrics["duration_ms"])
                metrics["num_turns"] = event.get("num_turns", 0)
                metrics["cost_usd"] = event.get("total_cost_usd", 0.0)
                usage = event.get("usage", {})
                metrics["input_tokens"] = usage.get("input_tokens", 0)
                metrics["output_tokens"] = usage.get("output_tokens", 0)
                metrics["cache_read_tokens"] = usage.get("cache_read_input_tokens", 0)
                metrics["cache_creation_tokens"] = usage.get("cache_creation_input_tokens", 0)
                continue

            # Assistant messages contain tool_use content blocks
            if etype == "assistant":
                msg = event.get("message", {})
                content = msg.get("content", [])
                for block in content:
                    if block.get("type") == "tool_use":
                        tool_name = block.get("name", "")
                        tool_input = block.get("input", {})
                        metrics["tool_calls"] += 1

                        if tool_name == "Read":
                            metrics["read_calls"] += 1
                            fp = tool_input.get("file_path", "")
                            if fp:
                                metrics["files_accessed"].add(fp)
                                if ".fmm/index.json" in fp or "fmm" in fp.lower() and "index" in fp.lower():
                                    metrics["manifest_accessed"] = True
                        elif tool_name == "Glob":
                            metrics["glob_calls"] += 1
                        elif tool_name == "Grep":
                            metrics["grep_calls"] += 1
                        elif tool_name == "Bash":
                            metrics["bash_calls"] += 1
                            cmd = tool_input.get("command", "")
                            if "fmm " in cmd or cmd.startswith("fmm"):
                                metrics["fmm_cli_calls"] += 1
                        elif tool_name.startswith("mcp__fmm__") or tool_name.startswith("fmm_"):
                            metrics["mcp_calls"] += 1

            # User messages with tool_result may contain file paths from Read results
            if etype == "user":
                msg = event.get("message", {})
                content = msg.get("content", [])
                for block in content:
                    if block.get("type") == "tool_result":
                        result_content = str(block.get("content", ""))
                        if ".fmm/index.json" in result_content:
                            metrics["manifest_accessed"] = True

    metrics["files_accessed"] = len(metrics["files_accessed"])
    return metrics


def main():
    all_runs = defaultdict(lambda: defaultdict(list))

    for cond in CONDITIONS:
        cond_dir = RESULTS_DIR / cond
        if not cond_dir.exists():
            continue
        for f in sorted(cond_dir.glob("*.jsonl")):
            parts = f.stem.rsplit("_run", 1)
            if len(parts) != 2:
                continue
            task_name = parts[0]
            metrics = parse_run(f)
            all_runs[cond][task_name].append(metrics)

    if not all_runs:
        print("No results found. Run experiments first.")
        sys.exit(1)

    # Summary table
    print()
    print("=" * 100)
    print("  exp15 RESULTS SUMMARY — Instruction Delivery Mechanism Comparison")
    print("=" * 100)
    print()

    header = (
        f"{'Condition':<20} {'Runs':>4} {'Tools':>6} {'Read':>5} {'Glob':>5} "
        f"{'Grep':>5} {'Bash':>5} {'MCP':>4} {'fmm':>4} {'Files':>5} "
        f"{'Mnfst':>5} {'InTok':>8} {'OutTok':>8} {'Cost':>7} {'ms':>8}"
    )
    print(header)
    print("-" * 100)

    cond_averages = {}

    for cond in CONDITIONS:
        if cond not in all_runs:
            continue

        all_metrics = []
        for task_name, runs in all_runs[cond].items():
            all_metrics.extend(runs)

        if not all_metrics:
            continue

        n = len(all_metrics)
        avg = {k: sum(m[k] for m in all_metrics) / n for k in [
            "tool_calls", "read_calls", "glob_calls", "grep_calls",
            "bash_calls", "mcp_calls", "fmm_cli_calls", "files_accessed",
            "input_tokens", "output_tokens", "duration_ms", "cost_usd",
        ]}
        avg["manifest_pct"] = sum(1 for m in all_metrics if m["manifest_accessed"]) / n * 100
        cond_averages[cond] = avg

        label = f"{cond}: {COND_LABELS[cond]}"
        mpct = f"{avg['manifest_pct']:.0f}%"
        print(
            f"{label:<20} {n:>4} {avg['tool_calls']:>6.1f} {avg['read_calls']:>5.1f} "
            f"{avg['glob_calls']:>5.1f} {avg['grep_calls']:>5.1f} {avg['bash_calls']:>5.1f} "
            f"{avg['mcp_calls']:>4.1f} {avg['fmm_cli_calls']:>4.1f} {avg['files_accessed']:>5.1f} "
            f"{mpct:>5} {avg['input_tokens']:>8.0f} {avg['output_tokens']:>8.0f} "
            f"${avg['cost_usd']:>5.2f} {avg['duration_ms']:>7.0f}"
        )

    # Per-task breakdown
    print()
    print("=" * 100)
    print("  PER-TASK BREAKDOWN (averages)")
    print("=" * 100)

    tasks_seen = sorted(set(t for cond_data in all_runs.values() for t in cond_data))

    for task in tasks_seen:
        print(f"\n  {'─'*60}")
        print(f"  Task: {task}")
        print(f"  {'Condition':<20} {'Runs':>4} {'Tools':>6} {'Read':>5} {'MCP':>4} {'fmm':>4} {'Mnfst':>5} {'Cost':>7}")
        print(f"  {'─'*60}")

        for cond in CONDITIONS:
            if cond not in all_runs or task not in all_runs[cond]:
                continue
            runs = all_runs[cond][task]
            n = len(runs)
            avg_tools = sum(m["tool_calls"] for m in runs) / n
            avg_reads = sum(m["read_calls"] for m in runs) / n
            avg_mcp = sum(m["mcp_calls"] for m in runs) / n
            avg_fmm = sum(m["fmm_cli_calls"] for m in runs) / n
            avg_cost = sum(m["cost_usd"] for m in runs) / n
            manifest_pct = sum(1 for m in runs if m["manifest_accessed"]) / n * 100
            label = f"{cond}: {COND_LABELS[cond]}"
            mpct = f"{manifest_pct:.0f}%"
            print(
                f"  {label:<20} {n:>4} {avg_tools:>6.1f} {avg_reads:>5.1f} "
                f"{avg_mcp:>4.1f} {avg_fmm:>4.1f} {mpct:>5} ${avg_cost:>5.2f}"
            )

    # Hypothesis evaluation
    if len(cond_averages) >= 2:
        print()
        print("=" * 100)
        print("  HYPOTHESIS EVALUATION")
        print("=" * 100)

        if "A" in cond_averages and "B" in cond_averages:
            a, b = cond_averages["A"], cond_averages["B"]
            denom = max(a["tool_calls"], 1)
            diff = abs(a["tool_calls"] - b["tool_calls"]) / denom * 100
            verdict = "CONFIRMED" if diff < 15 else "REJECTED"
            print(f"\n  H1: Skill ≈ CLAUDE.md (within 15%)")
            print(f"      A tool_calls={a['tool_calls']:.1f}  B tool_calls={b['tool_calls']:.1f}  diff={diff:.1f}% → {verdict}")
            print(f"      A manifest={a['manifest_pct']:.0f}%  B manifest={b['manifest_pct']:.0f}%")

        if "A" in cond_averages and "C" in cond_averages:
            a, c = cond_averages["A"], cond_averages["C"]
            verdict = "CONFIRMED" if c["manifest_pct"] < a["manifest_pct"] - 20 else "INCONCLUSIVE"
            print(f"\n  H2: MCP alone is insufficient")
            print(f"      A manifest={a['manifest_pct']:.0f}%  C manifest={c['manifest_pct']:.0f}% → {verdict}")
            print(f"      A tool_calls={a['tool_calls']:.1f}  C tool_calls={c['tool_calls']:.1f}")
            print(f"      C mcp_calls={c['mcp_calls']:.1f}")

        if "D" in cond_averages:
            d = cond_averages["D"]
            others = {c: cond_averages[c] for c in ["A", "B", "C"] if c in cond_averages}
            if others:
                best_other_tools = min(o["tool_calls"] for o in others.values())
                best_label = [c for c, o in others.items() if o["tool_calls"] == best_other_tools][0]
                verdict = "CONFIRMED" if d["tool_calls"] <= best_other_tools * 1.05 else "REJECTED"
                print(f"\n  H3: Skill + MCP is strictly best")
                print(f"      D tool_calls={d['tool_calls']:.1f}  best other ({best_label})={best_other_tools:.1f} → {verdict}")
                print(f"      D cost=${d['cost_usd']:.2f}  D manifest={d['manifest_pct']:.0f}%")

        if "B" in cond_averages and "D" in cond_averages:
            b, d = cond_averages["B"], cond_averages["D"]
            print(f"\n  H4: MCP enables better dependency queries")
            print(f"      B tool_calls={b['tool_calls']:.1f}  D tool_calls={d['tool_calls']:.1f}")
            print(f"      D mcp_calls={d['mcp_calls']:.1f}")

    # Raw JSON dump
    raw = {}
    for cond in CONDITIONS:
        if cond not in all_runs:
            continue
        raw[cond] = {}
        for task, runs in all_runs[cond].items():
            raw[cond][task] = [{k: v for k, v in m.items() if k != "files_accessed" or isinstance(v, int)} for m in runs]

    output_path = RESULTS_DIR / "exp15-summary.json"
    with open(output_path, "w") as f:
        json.dump(raw, f, indent=2, default=str)

    print()
    print(f"  Raw data: {output_path}")
    print()


if __name__ == "__main__":
    main()
