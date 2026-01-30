#!/usr/bin/env bash
set -euo pipefail

CONDITION="${CONDITION:-A}"
TASK_PROMPT="${TASK_PROMPT:-}"
TASK_ID="${TASK_ID:-unknown}"
CODEBASE="/experiment/workspace"
OUTFILE="/experiment/output.jsonl"

log() { echo "[$(date +%H:%M:%S)] $*" >&2; }

log "=== exp16: condition=$CONDITION task=$TASK_ID ==="

# 1. Copy codebase
log "Copying codebase..."
rsync -a --exclude='.git' --exclude='node_modules' --exclude='.fmm' /codebase/ "$CODEBASE/"
cd "$CODEBASE"
git config --global user.email "exp16@test.local"
git config --global user.name "exp16"
git init -q && git add -A && git commit -q -m "init" --allow-empty

# 2. Condition-specific setup
CLAUDE_ARGS=(
  --output-format stream-json --verbose
  --no-session-persistence
  --max-turns 30
  --model claude-sonnet-4-5-20250929
)

ALLOWED_TOOLS="Bash,Glob,Grep,Read,Write,Edit"

if [[ "$CONDITION" == "A" ]]; then
  # Vanilla: no fmm, no headers, no MCP, no skill
  log "Condition A: vanilla Claude"
  # Remove any FMM headers from files (shouldn't be any, but ensure clean)
  find "$CODEBASE" -type f \( -name "*.ts" -o -name "*.js" -o -name "*.py" -o -name "*.rs" -o -name "*.go" \) \
    -exec grep -l "--- FMM ---" {} \; 2>/dev/null | while read f; do
    log "WARNING: Removing FMM header from $f"
    # Remove FMM block (lines between --- FMM --- and --- END FMM ---)
    python3 -c "
import re, sys
content = open('$f').read()
content = re.sub(r'(//|#) --- FMM ---.*?(//|#) --- END FMM ---\n?', '', content, flags=re.DOTALL)
open('$f', 'w').write(content)
"
  done
  # Remove .fmm directory, MCP config, and CLAUDE.md
  rm -rf "$CODEBASE/.fmm"
  rm -f "$CODEBASE/.mcp.json"
  rm -rf "$CODEBASE/.claude"
  rm -f "$CODEBASE/CLAUDE.md"

  CLAUDE_ARGS+=(--allowedTools "$ALLOWED_TOOLS")

elif [[ "$CONDITION" == "B" ]]; then
  # FMM: headers in files + MCP server + skill
  log "Condition B: fmm (headers + MCP + skill)"

  # Generate FMM index + inject headers into all source files
  log "Running fmm generate..."
  fmm generate "$CODEBASE" 2>&1 | tail -5 >&2 || log "fmm generate warning (continuing)"

  # Verify headers were injected
  HEADER_COUNT=$( (grep -rl "--- FMM ---" "$CODEBASE" --include="*.ts" --include="*.js" 2>/dev/null || true) | wc -l)
  log "FMM headers injected: $HEADER_COUNT files"

  # Setup MCP config
  cat > "$CODEBASE/.mcp.json" <<'MCPEOF'
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["serve"]
    }
  }
}
MCPEOF

  # Setup skill via CLAUDE.md (avoid .claude/ dir which breaks API key detection)
  SKILL_CONTENT=""
  if [[ -f /usr/local/share/fmm/fmm-navigate.md ]]; then
    SKILL_CONTENT=$(cat /usr/local/share/fmm/fmm-navigate.md)
  elif [[ -f /usr/local/share/fmm-navigate.md ]]; then
    SKILL_CONTENT=$(cat /usr/local/share/fmm-navigate.md)
  fi
  if [[ -n "$SKILL_CONTENT" ]]; then
    cat > "$CODEBASE/CLAUDE.md" <<SKILLEOF
# FMM Code Navigation

$SKILL_CONTENT
SKILLEOF
    log "Skill injected via CLAUDE.md"
  fi

  ALLOWED_TOOLS="$ALLOWED_TOOLS,mcp__fmm__fmm_lookup_export,mcp__fmm__fmm_list_exports,mcp__fmm__fmm_file_info,mcp__fmm__fmm_dependency_graph,mcp__fmm__fmm_search,mcp__fmm__fmm_get_manifest,mcp__fmm__fmm_find_symbol,mcp__fmm__fmm_file_metadata,mcp__fmm__fmm_analyze_dependencies,mcp__fmm__fmm_project_overview"
  CLAUDE_ARGS+=(--allowedTools "$ALLOWED_TOOLS")
  CLAUDE_ARGS+=(--mcp-config .mcp.json)
fi

# 3. Clean up .claude dir (interferes with API key detection) + CLAUDE.md for A
rm -rf "$CODEBASE/.claude"
if [[ "$CONDITION" == "A" ]]; then
  rm -f "$CODEBASE/CLAUDE.md"
fi

# Recommit after setup changes
cd "$CODEBASE" && git add -A && git commit -q -m "setup" --allow-empty 2>/dev/null || true

# 4. Run Claude
log "Running Claude..."
log "  Prompt: $TASK_PROMPT"
log "  Args: ${CLAUDE_ARGS[*]}"

if claude -p "$TASK_PROMPT" "${CLAUDE_ARGS[@]}" > "$OUTFILE" 2>&1; then
  log "Claude finished successfully"
else
  log "Claude exited with code $?"
  log "Claude output:"
  cat "$OUTFILE" >&2 || true
fi

# 5. Output the JSONL
cat "$OUTFILE" 2>/dev/null || true

log "=== Done ==="
