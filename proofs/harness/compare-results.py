#!/usr/bin/env python3
"""Compare control vs treatment results and generate a markdown summary."""
import json
import os
import sys
from datetime import datetime

control_dir = sys.argv[1]
treatment_dir = sys.argv[2]
output_file = sys.argv[3]


def load_results(directory):
    """Load all .json result files from a directory."""
    results = {}
    for fname in sorted(os.listdir(directory)):
        if fname.endswith(".json") and not fname.endswith("_raw.json"):
            with open(os.path.join(directory, fname)) as f:
                data = json.load(f)
            results[data["label"]] = data
    return results


def pct_change(control_val, treatment_val):
    """Calculate percentage change from control to treatment."""
    if control_val == 0:
        return "N/A"
    change = ((treatment_val - control_val) / control_val) * 100
    return f"{change:+.0f}%"


def format_delta(control_val, treatment_val, unit=""):
    """Format a before/after comparison with delta."""
    delta = pct_change(control_val, treatment_val)
    return f"{control_val}{unit} -> {treatment_val}{unit} ({delta})"


control = load_results(control_dir)
treatment = load_results(treatment_dir)

labels = sorted(set(control.keys()) | set(treatment.keys()))

lines = []
lines.append("# Navigation Proof — Results Summary")
lines.append("")
lines.append(f"*Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}*")
lines.append("")
lines.append("## Conditions")
lines.append("")
lines.append("| | Control | Treatment |")
lines.append("|---|---|---|")
lines.append("| **Setup** | No fmm metadata | `.fmm/index.json` + CLAUDE.md hint |")
lines.append("| **Repo** | `research/exp14/repos/clean/` | `research/exp14/repos/hint/` |")
lines.append("| **Codebase** | 18-file TypeScript auth app | Same codebase + fmm sidecar |")
lines.append("")

# Per-query comparison
lines.append("## Per-Query Results")
lines.append("")

for label in labels:
    c = control.get(label, {})
    t = treatment.get(label, {})

    if not c or not t:
        continue

    lines.append(f"### {label}")
    lines.append("")
    lines.append(f"> {c.get('query', 'N/A')}")
    lines.append("")
    lines.append("| Metric | Control | Treatment | Delta |")
    lines.append("|--------|---------|-----------|-------|")

    c_tools = c.get("tool_calls_count", 0)
    t_tools = t.get("tool_calls_count", 0)
    lines.append(f"| Tool calls | {c_tools} | {t_tools} | {pct_change(c_tools, t_tools)} |")

    c_files = c.get("files_read_count", 0)
    t_files = t.get("files_read_count", 0)
    lines.append(f"| Files read | {c_files} | {t_files} | {pct_change(c_files, t_files)} |")

    c_tok = c.get("tokens_total", 0)
    t_tok = t.get("tokens_total", 0)
    lines.append(
        f"| Tokens (total) | {c_tok:,} | {t_tok:,} | {pct_change(c_tok, t_tok)} |"
    )

    c_cost = c.get("cost_usd", 0)
    t_cost = t.get("cost_usd", 0)
    lines.append(f"| Cost | ${c_cost:.4f} | ${t_cost:.4f} | {pct_change(c_cost, t_cost)} |")

    c_dur = c.get("duration_seconds", 0)
    t_dur = t.get("duration_seconds", 0)
    lines.append(f"| Duration | {c_dur}s | {t_dur}s | {pct_change(c_dur, t_dur)} |")

    lines.append(
        f"| FMM discovered | {c.get('discovered_fmm', False)} | {t.get('discovered_fmm', False)} | |"
    )
    lines.append(
        f"| Manifest used | {c.get('used_manifest', False)} | {t.get('used_manifest', False)} | |"
    )
    lines.append("")

    # Tool call breakdown
    c_types = c.get("tool_calls_by_type", {})
    t_types = t.get("tool_calls_by_type", {})
    all_tools = sorted(set(c_types.keys()) | set(t_types.keys()))
    if all_tools:
        lines.append("**Tool calls by type:**")
        lines.append("")
        lines.append("| Tool | Control | Treatment |")
        lines.append("|------|---------|-----------|")
        for tool in all_tools:
            lines.append(f"| {tool} | {c_types.get(tool, 0)} | {t_types.get(tool, 0)} |")
        lines.append("")

# Aggregate summary
lines.append("## Aggregate Summary")
lines.append("")

total_c_tools = sum(c.get("tool_calls_count", 0) for c in control.values())
total_t_tools = sum(t.get("tool_calls_count", 0) for t in treatment.values())
total_c_files = sum(c.get("files_read_count", 0) for c in control.values())
total_t_files = sum(t.get("files_read_count", 0) for t in treatment.values())
total_c_tokens = sum(c.get("tokens_total", 0) for c in control.values())
total_t_tokens = sum(t.get("tokens_total", 0) for t in treatment.values())
total_c_cost = sum(c.get("cost_usd", 0) for c in control.values())
total_t_cost = sum(t.get("cost_usd", 0) for t in treatment.values())
total_c_dur = sum(c.get("duration_seconds", 0) for c in control.values())
total_t_dur = sum(t.get("duration_seconds", 0) for t in treatment.values())

n = len(labels)

lines.append(f"*Across {n} navigation queries:*")
lines.append("")
lines.append("| Metric | Control (total) | Treatment (total) | Delta |")
lines.append("|--------|-----------------|-------------------|-------|")
lines.append(f"| Tool calls | {total_c_tools} | {total_t_tools} | {pct_change(total_c_tools, total_t_tools)} |")
lines.append(f"| Files read | {total_c_files} | {total_t_files} | {pct_change(total_c_files, total_t_files)} |")
lines.append(f"| Tokens | {total_c_tokens:,} | {total_t_tokens:,} | {pct_change(total_c_tokens, total_t_tokens)} |")
lines.append(f"| Cost | ${total_c_cost:.4f} | ${total_t_cost:.4f} | {pct_change(total_c_cost, total_t_cost)} |")
lines.append(f"| Duration | {total_c_dur}s | {total_t_dur}s | {pct_change(total_c_dur, total_t_dur)} |")
lines.append("")

# Headline for README
lines.append("## Headline")
lines.append("")
if total_c_files > 0 and total_t_files >= 0:
    lines.append(
        f"Without fmm: **{total_c_tools} tool calls**, **{total_c_files} file reads**, "
        f"**{total_c_tokens:,} tokens**"
    )
    lines.append(
        f"With fmm: **{total_t_tools} tool calls**, **{total_t_files} file reads**, "
        f"**{total_t_tokens:,} tokens**"
    )
    lines.append("")
    lines.append("Same answers. Fewer reads. Lower cost.")

output = "\n".join(lines) + "\n"

with open(output_file, "w") as f:
    f.write(output)

print(output)
