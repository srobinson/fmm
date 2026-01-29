#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TASK="Find all files that export authentication-related functions. List each file path and the specific exports."

echo "=== Running baseline experiments (9 runs) ==="
echo "Task: $TASK"
echo ""

for VARIANT in clean inline manifest; do
  for RUN in 1 2 3; do
    echo "--- $VARIANT run $RUN ---"
    "$SCRIPT_DIR/run-experiment.sh" "$VARIANT" "$TASK" "$RUN"
    echo ""
  done
done

echo "=== All baseline experiments complete ==="
echo ""

# Print summary
python3 - "$SCRIPT_DIR/results/baseline" << 'PYTHON'
import json
import os
import sys

results_dir = sys.argv[1]

print("=" * 80)
print("BASELINE EXPERIMENT SUMMARY")
print("=" * 80)

for variant in ["clean", "inline", "manifest"]:
    print(f"\n--- {variant.upper()} ---")
    for run in [1, 2, 3]:
        path = os.path.join(results_dir, f"{variant}_run{run}.json")
        if os.path.exists(path):
            with open(path) as f:
                data = json.load(f)
            print(f"  Run {run}: {data['tool_calls_count']} tools, {data['files_read_count']} files read, "
                  f"{data['tokens_in']}+{data['tokens_out']} tokens, {data['duration_seconds']}s, "
                  f"fmm={data['discovered_fmm']}, manifest={data['used_manifest']}, "
                  f"inline={data['noticed_inline_comments']}")
        else:
            print(f"  Run {run}: MISSING")

print("\n" + "=" * 80)
PYTHON
