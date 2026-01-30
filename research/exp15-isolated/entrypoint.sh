#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# fmm experiment entrypoint — runs a single isolated experiment
# ============================================================================
#
# Args: <condition> <task_idx> <run_num>
#   condition: A | B | C | D
#   task_idx:  0-3 (architecture, export-lookup, impact-analysis, dependency-map)
#   run_num:   1-3
#
# Env:
#   ANTHROPIC_API_KEY — required
#   CODEBASE_DIR     — mounted codebase (default: /codebase)
#   RESULTS_DIR      — output directory (default: /results)

CONDITION="${1:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"
TASK_IDX="${2:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"
RUN_NUM="${3:?Usage: entrypoint.sh <condition> <task_idx> <run_num>}"

CODEBASE_DIR="${CODEBASE_DIR:-/codebase}"
RESULTS_DIR="${RESULTS_DIR:-/results}"

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "ERROR: ANTHROPIC_API_KEY not set"
    exit 1
fi

# Task prompts (same as exp15)
TASKS=(
    "Describe the architecture of this project. What are the main modules and how do they interact?"
    "Find where the function createBillingSystem is defined and what module it belongs to."
    "If I change the function signature of validatePasswordStrength, what files would be affected?"
    "What external packages does this project depend on? List the top 10 by usage."
)
TASK_NAMES=("architecture" "export-lookup" "impact-analysis" "dependency-map")

TASK_PROMPT="${TASKS[$TASK_IDX]}"
TASK_NAME="${TASK_NAMES[$TASK_IDX]}"

log() { echo "  [$(date +%H:%M:%S)] $*"; }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Condition: $CONDITION | Task: $TASK_NAME | Run: $RUN_NUM"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ─── Environment info ────────────────────────────────────────────────────────
log "Environment:"
log "  fmm version:   $(fmm --version 2>&1 || echo 'NOT FOUND')"
log "  claude version: $(claude --version 2>&1 || echo 'NOT FOUND')"
log "  node version:   $(node --version 2>&1)"
log "  cwd:            $(pwd)"
log "  codebase:       $CODEBASE_DIR ($(ls "$CODEBASE_DIR" 2>/dev/null | wc -l) entries)"

# ─── Setup workspace ─────────────────────────────────────────────────────────
WORKSPACE="/experiment/workspace"
rm -rf "$WORKSPACE"
mkdir -p "$WORKSPACE"

log "Syncing codebase to workspace (excluding node_modules, .git, dist, .next, .fmm, .claude, CLAUDE.md)..."
rsync -a \
    --exclude 'node_modules' \
    --exclude '.git' \
    --exclude 'dist' \
    --exclude '.next' \
    --exclude '.fmm' \
    --exclude '.fmmrc.json' \
    --exclude '.mcp.json' \
    --exclude '.claude' \
    --exclude 'CLAUDE.md' \
    "$CODEBASE_DIR/" "$WORKSPACE/"

cd "$WORKSPACE"
log "Workspace ready: $(find . -type f | wc -l) files"

# Verify no CLAUDE.md leaked through
if [[ -f CLAUDE.md ]]; then
    log "WARNING: CLAUDE.md found in workspace ($(wc -c < CLAUDE.md) bytes) — removing"
    rm -f CLAUDE.md
fi

# Generate manifest (all conditions need it)
log "Running fmm init..."
fmm init 2>&1 | while read -r line; do log "  fmm: $line"; done

# Fix skill structure: Claude expects .claude/skills/<name>/SKILL.md not flat .md
if [[ -f .claude/skills/fmm-navigate.md ]]; then
    mkdir -p .claude/skills/fmm-navigate
    mv .claude/skills/fmm-navigate.md .claude/skills/fmm-navigate/SKILL.md
    log "Fixed skill structure → .claude/skills/fmm-navigate/SKILL.md"
fi

# ─── Apply condition-specific config ─────────────────────────────────────────
case "$CONDITION" in
    A)
        rm -rf .claude/skills/fmm-navigate
        rm -f .mcp.json
        cat > CLAUDE.md << 'SNIPPET'

## Code Navigation

This project uses fmm for LLM-optimized code navigation.

### Manifest Location
- `.fmm/index.json` contains metadata for all source files
- Query this before reading files to understand project structure

### Frontmatter Headers
Files contain `// --- FMM ---` headers with exports, imports, and LOC.
Read just the first 15 lines to understand what a file does.

### Quick Commands
```bash
fmm search --export <name>    # Find file by export
fmm search --imports <module> # Find files importing module
fmm search --loc ">500"       # Find large files
```
SNIPPET
        log "Condition A: CLAUDE.md snippet added, skill removed, MCP removed"
        ;;
    B)
        rm -f .mcp.json
        log "Condition B: Skill kept, MCP removed"
        ;;
    C)
        rm -rf .claude/skills/fmm-navigate
        log "Condition C: MCP kept, skill removed"
        ;;
    D)
        log "Condition D: Skill + MCP (full integration)"
        ;;
    *)
        echo "ERROR: Unknown condition: $CONDITION (expected A, B, C, D)"
        exit 1
        ;;
esac

# ─── Verify isolation ───────────────────────────────────────────────────────
log "Isolation verification:"
log "  ~/.claude exists:     $([ -d ~/.claude ] && echo YES || echo NO)"
log "  ~/.config exists:     $([ -d ~/.config ] && echo YES || echo NO)"
log "  CLAUDE.md exists:     $([ -f CLAUDE.md ] && echo YES || echo NO)"
log "  .mcp.json exists:     $([ -f .mcp.json ] && echo YES || echo NO)"
if [[ -f .mcp.json ]]; then
    log "  .mcp.json content:    $(cat .mcp.json)"
fi
log "  skill exists:         $([ -f .claude/skills/fmm-navigate/SKILL.md ] && echo YES || echo NO)"
if [[ -f .claude/skills/fmm-navigate/SKILL.md ]]; then
    log "  skill size:           $(wc -c < .claude/skills/fmm-navigate/SKILL.md) bytes"
fi
log "  manifest exists:      $([ -f .fmm/index.json ] && echo YES || echo NO)"
if [[ -f .fmm/index.json ]]; then
    log "  manifest size:        $(wc -c < .fmm/index.json) bytes"
    log "  manifest files:       $(python3 -c "import json; d=json.load(open('.fmm/index.json')); print(len(d.get('files',{})))" 2>/dev/null || echo 'parse error')"
fi

# ─── Run experiment ──────────────────────────────────────────────────────────
OUTDIR="$RESULTS_DIR/$CONDITION"
mkdir -p "$OUTDIR"
OUTFILE="$OUTDIR/${TASK_NAME}_run${RUN_NUM}.jsonl"
STDERRFILE="${OUTDIR}/${TASK_NAME}_run${RUN_NUM}.stderr.log"

# Allowed tools depend on condition
ALLOWED_TOOLS="Read,Glob,Grep,Bash"
if [[ "$CONDITION" == "C" || "$CONDITION" == "D" ]]; then
    ALLOWED_TOOLS="$ALLOWED_TOOLS,mcp__fmm__*"
fi

MCP_FLAGS=""
if [[ -f .mcp.json ]]; then
    MCP_FLAGS="--mcp-config .mcp.json"
fi

log "Claude CLI command:"
log "  claude --output-format stream-json --verbose \\"
log "    --allowedTools \"$ALLOWED_TOOLS\" \\"
log "    $MCP_FLAGS \\"
log "    --no-session-persistence \\"
log "    -p \"$TASK_PROMPT\""
log ""
log "Starting Claude..."

START_MS=$(python3 -c "import time; print(int(time.time()*1000))")

claude --output-format stream-json --verbose \
    --allowedTools "$ALLOWED_TOOLS" \
    $MCP_FLAGS \
    --no-session-persistence \
    -p "$TASK_PROMPT" \
    > "$OUTFILE" 2>"$STDERRFILE" || true

END_MS=$(python3 -c "import time; print(int(time.time()*1000))")
DURATION=$(( END_MS - START_MS ))

# Append metadata
echo "{\"_meta\":{\"condition\":\"$CONDITION\",\"task\":\"$TASK_NAME\",\"run\":$RUN_NUM,\"duration_ms\":$DURATION,\"isolated\":true}}" >> "$OUTFILE"

# ─── Post-run analysis ──────────────────────────────────────────────────────
log "Completed in ${DURATION}ms → $OUTFILE"
log "Output: $(wc -l < "$OUTFILE") lines, $(wc -c < "$OUTFILE") bytes"

# Extract key metrics from the JSONL
python3 -c "
import json, sys

mcp_servers = []
tools_available = []
tool_calls = {}
fmm_used = False

with open('$OUTFILE', encoding='utf-8', errors='replace') as f:
    for line in f:
        try:
            evt = json.loads(line.strip())
        except:
            continue
        if evt.get('type') == 'system' and evt.get('subtype') == 'init':
            mcp_servers = evt.get('mcp_servers', [])
            tools_available = [t for t in evt.get('tools', []) if 'fmm' in t.lower() or 'mcp' in t.lower()]
        if evt.get('type') == 'assistant':
            msg = evt.get('message', {})
            for block in msg.get('content', []):
                if block.get('type') == 'tool_use':
                    name = block.get('name', '')
                    tool_calls[name] = tool_calls.get(name, 0) + 1
                    if 'fmm' in name.lower():
                        fmm_used = True

fmm_calls = {k: v for k, v in tool_calls.items() if 'fmm' in k.lower()}
other_calls = {k: v for k, v in tool_calls.items() if 'fmm' not in k.lower()}

print(f'  MCP servers:    {mcp_servers}')
print(f'  FMM tools avail: {tools_available}')
print(f'  FMM tool calls:  {fmm_calls if fmm_calls else \"NONE\"}')
print(f'  Other calls:     {dict(sorted(other_calls.items(), key=lambda x: -x[1]))}')
print(f'  FMM USED:        {\"YES\" if fmm_used else \"NO\"}')
" 2>&1 | while read -r line; do log "$line"; done

# Check stderr for errors
if [[ -s "$STDERRFILE" ]]; then
    STDERR_SIZE=$(wc -c < "$STDERRFILE")
    log "Stderr output ($STDERR_SIZE bytes):"
    head -20 "$STDERRFILE" | while read -r line; do log "  stderr: $line"; done
fi
