#!/usr/bin/env python3
"""Compare isolated (Docker) vs non-isolated exp15 results.

Loads both datasets, produces side-by-side comparison tables,
calculates improvement ratios, and outputs combined summary JSON.

Usage:
    python3 compare-isolated.py
    python3 compare-isolated.py --isolated-dir ./results --baseline-dir ../exp15/results
"""

import json
import math
import sys
from pathlib import Path
from collections import defaultdict

SCRIPT_DIR = Path(__file__).parent
DEFAULT_ISOLATED_DIR = SCRIPT_DIR / "results"
DEFAULT_BASELINE_DIR = SCRIPT_DIR.parent / "exp15" / "results"

CONDITIONS = ["A", "B", "C", "D"]
COND_LABELS = {
    "A": "CLAUDE.md only",
    "B": "Skill only",
    "C": "MCP only",
    "D": "Skill + MCP",
}

METRIC_KEYS = [
    "tool_calls", "read_calls", "glob_calls", "grep_calls",
    "bash_calls", "mcp_calls", "fmm_cli_calls", "files_accessed",
    "input_tokens", "output_tokens", "duration_ms", "cost_usd",
]


def parse_run(filepath: Path) -> dict:
    """Parse a single run's stream-json output."""
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

            if "_meta" in event:
                metrics["duration_ms"] = event["_meta"].get("duration_ms", 0)
                continue

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

            if etype == "assistant":
                msg = event.get("message", {})
                for block in msg.get("content", []):
                    if block.get("type") == "tool_use":
                        tool_name = block.get("name", "")
                        tool_input = block.get("input", {})
                        metrics["tool_calls"] += 1

                        if tool_name == "Read":
                            metrics["read_calls"] += 1
                            fp = tool_input.get("file_path", "")
                            if fp:
                                metrics["files_accessed"].add(fp)
                                if ".fmm/index.json" in fp:
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
                            if "manifest" in tool_name or "index" in tool_name:
                                metrics["manifest_accessed"] = True

            if etype == "user":
                msg = event.get("message", {})
                for block in msg.get("content", []):
                    if block.get("type") == "tool_result":
                        if ".fmm/index.json" in str(block.get("content", "")):
                            metrics["manifest_accessed"] = True

    metrics["files_accessed"] = len(metrics["files_accessed"])
    return metrics


def load_dataset(results_dir: Path) -> dict:
    """Load all runs from a results directory into {condition: {task: [metrics]}}."""
    data = defaultdict(lambda: defaultdict(list))
    for cond in CONDITIONS:
        cond_dir = results_dir / cond
        if not cond_dir.exists():
            continue
        for f in sorted(cond_dir.glob("*.jsonl")):
            parts = f.stem.rsplit("_run", 1)
            if len(parts) != 2:
                continue
            task_name = parts[0]
            metrics = parse_run(f)
            data[cond][task_name].append(metrics)
    return data


def compute_averages(data: dict) -> dict:
    """Compute per-condition averages."""
    averages = {}
    for cond in CONDITIONS:
        if cond not in data:
            continue
        all_metrics = []
        for runs in data[cond].values():
            all_metrics.extend(runs)
        if not all_metrics:
            continue
        n = len(all_metrics)
        avg = {k: sum(m[k] for m in all_metrics) / n for k in METRIC_KEYS}
        avg["manifest_pct"] = sum(1 for m in all_metrics if m["manifest_accessed"]) / n * 100
        avg["n"] = n
        averages[cond] = avg
    return averages


def stddev(values: list) -> float:
    """Sample standard deviation (Bessel's correction)."""
    if len(values) < 2:
        return 0.0
    mean = sum(values) / len(values)
    variance = sum((x - mean) ** 2 for x in values) / (len(values) - 1)
    return math.sqrt(variance)


def compute_stats(data: dict) -> dict:
    """Compute per-condition mean + stddev for key metrics."""
    stats = {}
    for cond in CONDITIONS:
        if cond not in data:
            continue
        all_metrics = []
        for runs in data[cond].values():
            all_metrics.extend(runs)
        if not all_metrics:
            continue
        n = len(all_metrics)
        result = {}
        for k in ["tool_calls", "cost_usd", "duration_ms", "input_tokens"]:
            values = [m[k] for m in all_metrics]
            mean = sum(values) / n
            sd = stddev(values)
            result[k] = {"mean": mean, "stddev": sd, "n": n}
        result["manifest_pct"] = sum(1 for m in all_metrics if m["manifest_accessed"]) / n * 100
        stats[cond] = result
    return stats


def print_comparison(baseline_avgs: dict, isolated_avgs: dict):
    """Print side-by-side comparison table."""
    print()
    print("=" * 110)
    print("  ISOLATED vs NON-ISOLATED COMPARISON")
    print("=" * 110)
    print()

    header = (
        f"{'Condition':<20} │ {'Tools (base)':>11} {'Tools (iso)':>11} {'Δ%':>6} │ "
        f"{'Cost (base)':>11} {'Cost (iso)':>10} {'Δ%':>6} │ "
        f"{'Mnfst (b)':>9} {'Mnfst (i)':>9}"
    )
    print(header)
    print("─" * 110)

    for cond in CONDITIONS:
        base = baseline_avgs.get(cond)
        iso = isolated_avgs.get(cond)
        label = f"{cond}: {COND_LABELS[cond]}"

        if base and iso:
            tool_delta = ((iso["tool_calls"] - base["tool_calls"]) / max(base["tool_calls"], 1)) * 100
            cost_delta = ((iso["cost_usd"] - base["cost_usd"]) / max(base["cost_usd"], 0.01)) * 100

            print(
                f"{label:<20} │ "
                f"{base['tool_calls']:>11.1f} {iso['tool_calls']:>11.1f} {tool_delta:>+5.0f}% │ "
                f"${base['cost_usd']:>10.2f} ${iso['cost_usd']:>9.2f} {cost_delta:>+5.0f}% │ "
                f"{base['manifest_pct']:>8.0f}% {iso['manifest_pct']:>8.0f}%"
            )
        elif base:
            print(
                f"{label:<20} │ "
                f"{base['tool_calls']:>11.1f} {'—':>11} {'—':>6} │ "
                f"${base['cost_usd']:>10.2f} {'—':>10} {'—':>6} │ "
                f"{base['manifest_pct']:>8.0f}% {'—':>9}"
            )
        elif iso:
            print(
                f"{label:<20} │ "
                f"{'—':>11} {iso['tool_calls']:>11.1f} {'—':>6} │ "
                f"{'—':>11} ${iso['cost_usd']:>9.2f} {'—':>6} │ "
                f"{'—':>9} {iso['manifest_pct']:>8.0f}%"
            )

    print()


def print_headline(baseline_avgs: dict, isolated_avgs: dict):
    """Print the headline improvement metric."""
    print("=" * 110)
    print("  HEADLINE METRICS")
    print("=" * 110)
    print()

    # Best condition (D) isolated vs worst non-isolated baseline (A)
    if "D" in isolated_avgs and "A" in baseline_avgs:
        base_a = baseline_avgs["A"]
        iso_d = isolated_avgs["D"]
        tool_improvement = (base_a["tool_calls"] - iso_d["tool_calls"]) / max(iso_d["tool_calls"], 1) * 100
        cost_improvement = (base_a["cost_usd"] - iso_d["cost_usd"]) / max(iso_d["cost_usd"], 0.01) * 100

        print(f"  Skill+MCP (isolated) vs CLAUDE.md (non-isolated):")
        print(f"    Tool calls:  {base_a['tool_calls']:.1f} → {iso_d['tool_calls']:.1f}  ({tool_improvement:+.0f}% improvement)")
        print(f"    Cost:        ${base_a['cost_usd']:.2f} → ${iso_d['cost_usd']:.2f}  ({cost_improvement:+.0f}% improvement)")
        print()

    # Same condition isolated vs non-isolated
    for cond in CONDITIONS:
        if cond in baseline_avgs and cond in isolated_avgs:
            base = baseline_avgs[cond]
            iso = isolated_avgs[cond]
            tool_delta = (base["tool_calls"] - iso["tool_calls"]) / max(iso["tool_calls"], 1) * 100
            print(f"  {cond} ({COND_LABELS[cond]}): {base['tool_calls']:.1f} → {iso['tool_calls']:.1f} tool calls ({tool_delta:+.0f}%)")

    print()


def print_stats(label: str, stats: dict):
    """Print statistical summary."""
    print(f"\n  {label}:")
    print(f"  {'Condition':<20} {'Tools':>14} {'Cost':>14} {'Duration':>14}")
    print(f"  {'─'*62}")

    for cond in CONDITIONS:
        if cond not in stats:
            continue
        s = stats[cond]
        name = f"{cond}: {COND_LABELS[cond]}"
        print(
            f"  {name:<20} "
            f"{s['tool_calls']['mean']:>6.1f} ±{s['tool_calls']['stddev']:>5.1f} "
            f"${s['cost_usd']['mean']:>5.2f} ±{s['cost_usd']['stddev']:>5.2f} "
            f"{s['duration_ms']['mean']/1000:>6.1f}s ±{s['duration_ms']['stddev']/1000:>5.1f}s"
        )


def save_combined_json(baseline_data: dict, isolated_data: dict, output_path: Path):
    """Save combined summary JSON with both datasets."""
    def serialize_dataset(data):
        result = {}
        for cond in CONDITIONS:
            if cond not in data:
                continue
            result[cond] = {}
            for task, runs in data[cond].items():
                result[cond][task] = [
                    {k: v for k, v in m.items() if not isinstance(v, set)}
                    for m in runs
                ]
        return result

    combined = {
        "baseline": serialize_dataset(baseline_data),
        "isolated": serialize_dataset(isolated_data),
        "baseline_averages": {
            cond: avg for cond, avg in compute_averages(baseline_data).items()
        },
        "isolated_averages": {
            cond: avg for cond, avg in compute_averages(isolated_data).items()
        },
    }

    with open(output_path, "w") as f:
        json.dump(combined, f, indent=2, default=str)

    print(f"  Combined data: {output_path}")


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Compare isolated vs non-isolated experiment results")
    parser.add_argument("--isolated-dir", type=Path, default=DEFAULT_ISOLATED_DIR)
    parser.add_argument("--baseline-dir", type=Path, default=DEFAULT_BASELINE_DIR)
    args = parser.parse_args()

    baseline_data = load_dataset(args.baseline_dir)
    isolated_data = load_dataset(args.isolated_dir)

    if not baseline_data and not isolated_data:
        print("No results found in either directory.")
        print(f"  Baseline: {args.baseline_dir}")
        print(f"  Isolated: {args.isolated_dir}")
        sys.exit(1)

    baseline_avgs = compute_averages(baseline_data)
    isolated_avgs = compute_averages(isolated_data)

    print()
    print("╔══════════════════════════════════════════════════════════════════════╗")
    print("║  exp15: Isolated vs Non-Isolated Results Comparison                ║")
    print("╚══════════════════════════════════════════════════════════════════════╝")

    if baseline_data:
        baseline_stats = compute_stats(baseline_data)
        print_stats("Non-Isolated (baseline) — mean ± stddev", baseline_stats)

    if isolated_data:
        isolated_stats = compute_stats(isolated_data)
        print_stats("Isolated (Docker) — mean ± stddev", isolated_stats)

    if baseline_avgs and isolated_avgs:
        print_comparison(baseline_avgs, isolated_avgs)
        print_headline(baseline_avgs, isolated_avgs)

    # Save combined JSON
    output_path = args.isolated_dir / "exp15-summary.json"
    save_combined_json(baseline_data, isolated_data, output_path)

    print()


if __name__ == "__main__":
    main()
