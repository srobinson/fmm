# Automated Claude Code Benchmarking Research

## Overview

This document outlines the tools, flags, and strategies for running automated Claude Code benchmarking sessions to compare control vs FMM (Frontmatter) variants.

---

## 1. Claude Code CLI Flags for Non-Interactive Usage

### Core Automation Flags

| Flag | Description |
|------|-------------|
| `-p, --print` | Non-interactive mode - prints response and exits. Essential for automation. |
| `--output-format <format>` | `text` (default), `json` (structured), or `stream-json` (real-time) |
| `--max-turns <n>` | Limit autonomous actions (prevents runaway processes) |
| `--allowedTools <tools>` | Restrict available tools (e.g., `"Read,Glob,Grep"`) |
| `--disallowedTools <tools>` | Block specific tools |
| `--tools <tools>` | Specify exact tool list, use `""` to disable all |
| `--session-id <uuid>` | Use specific session ID for tracking |
| `--no-session-persistence` | Don't save session to disk (only with `--print`) |
| `--max-budget-usd <amount>` | Cost cap per session (only with `--print`) |
| `--model <model>` | Specify model (e.g., `opus`, `sonnet`, `claude-opus-4-5-20251101`) |
| `--system-prompt <prompt>` | Custom system prompt |
| `--append-system-prompt <prompt>` | Add to default system prompt |
| `--dangerously-skip-permissions` | Bypass permission checks (for sandboxed environments only) |
| `--verbose` | Required for `stream-json` output |

### Basic Example

```bash
claude -p "Explain the main function" \
  --output-format json \
  --max-turns 5 \
  --allowedTools "Read,Glob,Grep" \
  --max-budget-usd 1.00
```

---

## 2. Output Formats Explained

### JSON Output (`--output-format json`)

Returns a single JSON object upon completion:

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "duration_ms": 7429,
  "duration_api_ms": 9638,
  "num_turns": 2,
  "result": "The response text...",
  "session_id": "8161fc97-919e-4f0d-b25d-ea631754f77b",
  "total_cost_usd": 0.093718,
  "usage": {
    "input_tokens": 4,
    "cache_creation_input_tokens": 13154,
    "cache_read_input_tokens": 13025,
    "output_tokens": 174,
    "server_tool_use": {
      "web_search_requests": 0,
      "web_fetch_requests": 0
    }
  },
  "modelUsage": {
    "claude-opus-4-5-20251101": {
      "inputTokens": 4,
      "outputTokens": 174,
      "cacheReadInputTokens": 13025,
      "cacheCreationInputTokens": 13154,
      "costUSD": 0.093095
    }
  },
  "permission_denials": []
}
```

### Stream JSON Output (`--output-format stream-json --verbose`)

Returns newline-delimited JSON objects in real-time:

1. **Init message** (`type: "system", subtype: "init"`): Contains session setup, available tools, MCP servers
2. **Assistant messages** (`type: "assistant"`): Contains tool_use blocks with tool names and inputs
3. **User messages** (`type: "user"`): Contains tool_result blocks with outputs
4. **Result message** (`type: "result"`): Final summary with costs and metrics

**Counting Tool Calls:**
```bash
claude -p "analyze codebase" --output-format stream-json --verbose 2>&1 | \
  grep '"type":"tool_use"' | wc -l
```

---

## 3. Capturing Tool Call Counts Programmatically

### Method 1: Parse Stream JSON

```python
import json
import subprocess

def run_claude_and_count_tools(prompt: str, cwd: str) -> dict:
    result = subprocess.run(
        ["claude", "-p", prompt, "--output-format", "stream-json", "--verbose"],
        capture_output=True,
        text=True,
        cwd=cwd
    )

    tool_calls = []
    final_result = None

    for line in result.stdout.strip().split('\n'):
        if not line:
            continue
        data = json.loads(line)

        if data.get("type") == "assistant":
            message = data.get("message", {})
            for content in message.get("content", []):
                if content.get("type") == "tool_use":
                    tool_calls.append({
                        "name": content.get("name"),
                        "input": content.get("input")
                    })

        if data.get("type") == "result":
            final_result = data

    return {
        "tool_calls": tool_calls,
        "tool_count": len(tool_calls),
        "tools_by_name": _count_by_name(tool_calls),
        "total_cost_usd": final_result.get("total_cost_usd"),
        "num_turns": final_result.get("num_turns"),
        "duration_ms": final_result.get("duration_ms"),
        "result": final_result.get("result")
    }

def _count_by_name(tool_calls):
    counts = {}
    for tc in tool_calls:
        name = tc["name"]
        counts[name] = counts.get(name, 0) + 1
    return counts
```

### Method 2: Use `num_turns` from JSON Output

The `num_turns` field indicates how many agentic turns occurred. Each turn typically involves tool calls:

```bash
claude -p "query" --output-format json | jq '.num_turns'
```

### Method 3: Track Specific Tools

```bash
# Extract all tool names used
claude -p "query" --output-format stream-json --verbose 2>&1 | \
  jq -r 'select(.type == "assistant") | .message.content[]? | select(.type == "tool_use") | .name' | \
  sort | uniq -c
```

---

## 4. Session Isolation Strategies

### Strategy 1: Fresh Session IDs

```bash
SESSION_ID=$(uuidgen)
claude -p "query" --session-id "$SESSION_ID" --no-session-persistence
```

### Strategy 2: Isolated Working Directories

```bash
# Clone repo into temp directory for each test
WORK_DIR=$(mktemp -d)
git clone --depth 1 "$REPO_URL" "$WORK_DIR/repo"
cd "$WORK_DIR/repo"
claude -p "query" --output-format json
rm -rf "$WORK_DIR"
```

### Strategy 3: Docker/Container Isolation

Using claude-code-sandbox (https://github.com/textcortex/claude-code-sandbox):

```bash
# Start isolated container
docker run -d --name test-run-1 \
  -v $(pwd)/repo:/workspace \
  claude-code-sandbox

# Run Claude inside
docker exec test-run-1 claude -p "query" \
  --dangerously-skip-permissions \
  --output-format json
```

### Strategy 4: Git Worktrees for Parallel Tests

```bash
# Create isolated worktrees
git worktree add ../test-control main
git worktree add ../test-fmm fmm-branch

# Run parallel tests
(cd ../test-control && claude -p "query" --output-format json > control.json) &
(cd ../test-fmm && claude -p "query" --output-format json > fmm.json) &
wait
```

---

## 5. Existing Benchmarking Frameworks

### For LLM Output Evaluation

| Framework | Best For | Key Features |
|-----------|----------|--------------|
| DeepEval | Pytest-style LLM testing | 50+ metrics, tool call evaluation, CI/CD integration |
| RAGAS | RAG pipelines | Retrieval quality metrics |
| LangSmith | LangChain apps | Automated regression testing |
| Opik | CI/CD workflows | Unit-test style API |
| MLflow | ML lifecycle | Experiment tracking |

### For Code Agent Benchmarks

| Benchmark | Description |
|-----------|-------------|
| SWE-bench | Real GitHub issues (2,294 tasks) |
| SWE-bench Lite | Filtered easier subset |
| SWE-bench Verified | Contamination-free subset |
| SWE-bench Pro | Long-horizon tasks |

### DeepEval Tool Call Evaluation Example

```python
from deepeval import evaluate
from deepeval.test_case import LLMTestCase
from deepeval.metrics import ToolCorrectnessMetric

test_case = LLMTestCase(
    input="Find all TypeScript files in src/",
    actual_output="Found 16 files...",
    tools_called=["Glob"],  # Actual tools used
    expected_tools=["Glob"]  # Expected tools
)

metric = ToolCorrectnessMetric()
evaluate([test_case], [metric])
```

---

## 6. Token/Cost Tracking Per Session

### From JSON Output

```python
def extract_costs(json_result: dict) -> dict:
    return {
        "total_cost_usd": json_result.get("total_cost_usd"),
        "duration_ms": json_result.get("duration_ms"),
        "duration_api_ms": json_result.get("duration_api_ms"),
        "usage": {
            "input_tokens": json_result["usage"]["input_tokens"],
            "output_tokens": json_result["usage"]["output_tokens"],
            "cache_read_tokens": json_result["usage"].get("cache_read_input_tokens", 0),
            "cache_creation_tokens": json_result["usage"].get("cache_creation_input_tokens", 0),
        },
        "model_breakdown": json_result.get("modelUsage", {})
    }
```

### Per-Model Cost Breakdown

The `modelUsage` field provides costs split by model:

```json
{
  "modelUsage": {
    "claude-haiku-4-5-20251001": {
      "inputTokens": 123,
      "outputTokens": 100,
      "costUSD": 0.000623
    },
    "claude-opus-4-5-20251101": {
      "inputTokens": 4,
      "outputTokens": 174,
      "cacheReadInputTokens": 13025,
      "cacheCreationInputTokens": 13154,
      "costUSD": 0.093095
    }
  }
}
```

### Cost Aggregation Script

```bash
#!/bin/bash
# aggregate_costs.sh - Sum costs from multiple test runs

total=0
for f in results/*.json; do
    cost=$(jq -r '.total_cost_usd // 0' "$f")
    total=$(echo "$total + $cost" | bc)
done
echo "Total cost: \$${total}"
```

---

## 7. Timeout and Error Handling

### Timeout Configuration

```bash
# Set timeout via environment
export BASH_DEFAULT_TIMEOUT_MS=300000  # 5 minutes
export BASH_MAX_TIMEOUT_MS=600000      # 10 minutes

# Or use external timeout
timeout 300 claude -p "query" --output-format json
```

### Error Result Types

The `subtype` field indicates completion status:

| Subtype | Meaning |
|---------|---------|
| `success` | Completed normally |
| `error_max_turns` | Hit `--max-turns` limit |
| `error_budget` | Exceeded `--max-budget-usd` |
| `error` | General error |

### Retry Logic with Exponential Backoff

```python
import time
import subprocess
import json

def run_with_retry(prompt: str, max_retries: int = 3) -> dict:
    for attempt in range(max_retries):
        try:
            result = subprocess.run(
                ["claude", "-p", prompt, "--output-format", "json"],
                capture_output=True,
                text=True,
                timeout=300  # 5 minute timeout
            )
            data = json.loads(result.stdout)

            if not data.get("is_error"):
                return data

            # Retry on certain errors
            if data.get("subtype") == "error_max_turns":
                return data  # Don't retry max turns

        except subprocess.TimeoutExpired:
            pass
        except json.JSONDecodeError:
            pass

        # Exponential backoff
        delay = 2 ** attempt * 10  # 10s, 20s, 40s
        time.sleep(delay)

    raise Exception(f"Failed after {max_retries} retries")
```

---

## 8. Proposed Benchmark Test Runner Architecture

### High-Level Design

```
+-------------------------------------------------------------+
|                    Benchmark Runner                          |
+-------------------------------------------------------------+
|  1. Setup Phase                                              |
|     - Clone target repo                                      |
|     - Create isolated work directories                       |
|     - Generate control vs FMM variants                       |
+-------------------------------------------------------------+
|  2. Execution Phase                                          |
|     - Run Claude with prompt                                 |
|     - Capture stream-json output                             |
|     - Track tool calls, costs, timing                        |
+-------------------------------------------------------------+
|  3. Evaluation Phase                                         |
|     - Parse results                                          |
|     - Check accuracy against expected output                 |
|     - Calculate efficiency metrics                           |
+-------------------------------------------------------------+
|  4. Reporting Phase                                          |
|     - Aggregate results                                      |
|     - Generate comparison tables                             |
|     - Identify statistical significance                      |
+-------------------------------------------------------------+
```

### Core Data Model

```typescript
interface BenchmarkResult {
  // Identification
  testId: string;
  variant: "control" | "fmm";
  repoUrl: string;
  prompt: string;

  // Execution metrics
  toolCalls: ToolCall[];
  toolCallCount: number;
  toolCallsByType: Record<string, number>;
  filesRead: string[];
  filesWritten: string[];

  // Cost metrics
  totalCostUsd: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;

  // Timing
  durationMs: number;
  durationApiMs: number;
  numTurns: number;

  // Output
  response: string;

  // Evaluation
  accuracy: number;  // 0-1 score
  correctnessNotes: string[];
}

interface ToolCall {
  name: string;
  input: Record<string, unknown>;
  output?: string;
  durationMs?: number;
}
```

### Example Test Runner Script

```python
#!/usr/bin/env python3
"""
FMM Benchmark Test Runner

Compares Claude Code performance with and without FMM frontmatter.
"""

import json
import subprocess
import tempfile
import shutil
import uuid
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import List, Optional
import time

@dataclass
class BenchmarkResult:
    test_id: str
    variant: str
    prompt: str
    tool_calls: List[dict]
    tool_count: int
    files_read: List[str]
    total_cost_usd: float
    duration_ms: int
    num_turns: int
    response: str
    is_error: bool
    error_type: Optional[str]

class FMMBenchmarkRunner:
    def __init__(self, repo_url: str, prompts: List[str]):
        self.repo_url = repo_url
        self.prompts = prompts
        self.results: List[BenchmarkResult] = []

    def run_all(self) -> List[BenchmarkResult]:
        """Run all benchmark tests."""
        for prompt in self.prompts:
            # Run control (no FMM)
            control_result = self._run_single(prompt, "control", with_fmm=False)
            self.results.append(control_result)

            # Run FMM variant
            fmm_result = self._run_single(prompt, "fmm", with_fmm=True)
            self.results.append(fmm_result)

        return self.results

    def _run_single(self, prompt: str, variant: str, with_fmm: bool) -> BenchmarkResult:
        """Run a single benchmark test."""
        test_id = str(uuid.uuid4())[:8]
        work_dir = Path(tempfile.mkdtemp())

        try:
            # Clone repo
            subprocess.run(
                ["git", "clone", "--depth", "1", self.repo_url, str(work_dir / "repo")],
                check=True,
                capture_output=True
            )

            repo_dir = work_dir / "repo"

            # Optionally generate FMM frontmatter
            if with_fmm:
                self._generate_fmm(repo_dir)

            # Run Claude
            result = subprocess.run(
                [
                    "claude", "-p", prompt,
                    "--output-format", "stream-json",
                    "--verbose",
                    "--max-turns", "20",
                    "--max-budget-usd", "2.00"
                ],
                capture_output=True,
                text=True,
                cwd=str(repo_dir),
                timeout=300
            )

            # Parse results
            return self._parse_stream_json(test_id, variant, prompt, result.stdout)

        finally:
            shutil.rmtree(work_dir, ignore_errors=True)

    def _generate_fmm(self, repo_dir: Path):
        """Generate FMM frontmatter for the repo."""
        # Run fmm generate command
        subprocess.run(
            ["fmm", "generate"],
            cwd=str(repo_dir),
            capture_output=True
        )

    def _parse_stream_json(self, test_id: str, variant: str, prompt: str, output: str) -> BenchmarkResult:
        """Parse stream-json output into BenchmarkResult."""
        tool_calls = []
        files_read = []
        final_result = None

        for line in output.strip().split('\n'):
            if not line:
                continue
            try:
                data = json.loads(line)
            except json.JSONDecodeError:
                continue

            if data.get("type") == "assistant":
                for content in data.get("message", {}).get("content", []):
                    if content.get("type") == "tool_use":
                        tool_call = {
                            "name": content.get("name"),
                            "input": content.get("input", {})
                        }
                        tool_calls.append(tool_call)

                        # Track file reads
                        if content.get("name") == "Read":
                            files_read.append(content.get("input", {}).get("file_path", ""))

            if data.get("type") == "result":
                final_result = data

        return BenchmarkResult(
            test_id=test_id,
            variant=variant,
            prompt=prompt,
            tool_calls=tool_calls,
            tool_count=len(tool_calls),
            files_read=files_read,
            total_cost_usd=final_result.get("total_cost_usd", 0) if final_result else 0,
            duration_ms=final_result.get("duration_ms", 0) if final_result else 0,
            num_turns=final_result.get("num_turns", 0) if final_result else 0,
            response=final_result.get("result", "") if final_result else "",
            is_error=final_result.get("is_error", True) if final_result else True,
            error_type=final_result.get("subtype") if final_result and final_result.get("is_error") else None
        )

    def generate_report(self) -> str:
        """Generate comparison report."""
        report = ["# FMM Benchmark Results\n"]

        # Group by prompt
        prompts = {}
        for r in self.results:
            if r.prompt not in prompts:
                prompts[r.prompt] = {}
            prompts[r.prompt][r.variant] = r

        for prompt, variants in prompts.items():
            report.append(f"\n## Prompt: {prompt[:50]}...\n")
            report.append("| Metric | Control | FMM | Difference |")
            report.append("|--------|---------|-----|------------|")

            control = variants.get("control")
            fmm = variants.get("fmm")

            if control and fmm:
                tool_diff = fmm.tool_count - control.tool_count
                cost_diff = fmm.total_cost_usd - control.total_cost_usd
                time_diff = fmm.duration_ms - control.duration_ms

                report.append(f"| Tool Calls | {control.tool_count} | {fmm.tool_count} | {tool_diff:+d} |")
                report.append(f"| Cost (USD) | ${control.total_cost_usd:.4f} | ${fmm.total_cost_usd:.4f} | ${cost_diff:+.4f} |")
                report.append(f"| Duration (ms) | {control.duration_ms} | {fmm.duration_ms} | {time_diff:+d} |")
                report.append(f"| Turns | {control.num_turns} | {fmm.num_turns} | {fmm.num_turns - control.num_turns:+d} |")

        return "\n".join(report)


if __name__ == "__main__":
    runner = FMMBenchmarkRunner(
        repo_url="https://github.com/pmndrs/zustand.git",
        prompts=[
            "What state management pattern does this library implement?",
            "How does the middleware system work?",
            "Find all exported functions from the main entry point",
        ]
    )

    results = runner.run_all()
    report = runner.generate_report()
    print(report)

    # Save detailed results
    with open("benchmark_results.json", "w") as f:
        json.dump([asdict(r) for r in results], f, indent=2)
```

---

## 9. Key Metrics to Capture

### Efficiency Metrics

| Metric | Description | Goal with FMM |
|--------|-------------|---------------|
| `tool_count` | Total tool invocations | Lower |
| `files_read` | Unique files accessed | Lower (more targeted) |
| `num_turns` | Agent loop iterations | Lower |
| `duration_ms` | Wall clock time | Lower |
| `total_cost_usd` | API costs | Lower |

### Quality Metrics

| Metric | Description | Measurement |
|--------|-------------|-------------|
| Accuracy | Correctness of response | Manual review or LLM-as-judge |
| Completeness | All aspects addressed | Checklist scoring |
| Relevance | Response matches query | Embedding similarity |

### Tool Usage Distribution

Track which tools are used and how often:

```python
def analyze_tool_distribution(results: List[BenchmarkResult]) -> dict:
    control_tools = {}
    fmm_tools = {}

    for r in results:
        target = control_tools if r.variant == "control" else fmm_tools
        for tc in r.tool_calls:
            name = tc["name"]
            target[name] = target.get(name, 0) + 1

    return {
        "control": control_tools,
        "fmm": fmm_tools,
        "reduction": {
            tool: control_tools.get(tool, 0) - fmm_tools.get(tool, 0)
            for tool in set(control_tools) | set(fmm_tools)
        }
    }
```

---

## 10. Sources and References

### Claude Code Documentation
- CLI Reference: https://code.claude.com/docs/en/cli-reference
- Sandboxing Guide: https://code.claude.com/docs/en/sandboxing
- Cost Management: https://code.claude.com/docs/en/costs
- Claude Agent SDK Overview: https://platform.claude.com/docs/en/agent-sdk/overview

### Evaluation Frameworks
- DeepEval - LLM Evaluation Framework: https://github.com/confident-ai/deepeval
- SWE-bench Repository: https://github.com/SWE-bench/SWE-bench
- LangSmith: https://smith.langchain.com/
- RAGAS: https://github.com/explodinggradients/ragas

### Sandbox Solutions
- Claude Code Sandbox (Docker): https://github.com/textcortex/claude-code-sandbox
- ClaudeBox (Micro-VMs): https://github.com/boxlite-ai/claudebox
- E2B Sandbox Guide: https://e2b.dev/blog/python-guide-run-claude-code-in-an-e2b-sandbox

### Community Resources
- Claude Code CLI Cheatsheet: https://shipyard.build/blog/claude-code-cheat-sheet/
- Claude Code Usage Monitor: https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor
- Claude Code is Programmable: https://github.com/disler/claude-code-is-programmable

---

## 11. Next Steps

1. **Build minimal test runner** - Start with the Python script above
2. **Define test corpus** - Select representative prompts/tasks
3. **Establish baselines** - Run control tests first
4. **Iterate on FMM generation** - Tune frontmatter quality
5. **Statistical analysis** - Use t-tests for significance
6. **CI integration** - Automate benchmark runs on changes
