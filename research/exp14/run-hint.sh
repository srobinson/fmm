#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TASK="Find all files that export authentication-related functions. List each file path and the specific exports."

echo "=== Running hint experiments (3 runs) ==="
echo "Task: $TASK"
echo ""

for RUN in 1 2 3; do
  echo "--- hint run $RUN ---"
  "$SCRIPT_DIR/run-experiment.sh" "hint" "$TASK" "$RUN"
  echo ""
done

echo "=== All hint experiments complete ==="
echo ""

# Print summary
python3 - "$SCRIPT_DIR/results/hint" << 'PYTHON'
import json
import os
import sys

results_dir = sys.argv[1]

print("=" * 80)
print("HINT EXPERIMENT SUMMARY")
print("=" * 80)

for run in [1, 2, 3]:
    path = os.path.join(results_dir, f"hint_run{run}.json")
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
