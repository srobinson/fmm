#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FMM_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Setting up exp16-cost experiment..."

# 1. Copy fmm source
echo "1. Copying fmm source to fmm-src/..."
rm -rf "$SCRIPT_DIR/fmm-src"
mkdir -p "$SCRIPT_DIR/fmm-src/src" "$SCRIPT_DIR/fmm-src/benches" "$SCRIPT_DIR/fmm-src/docs"
rsync -a --include='*.rs' --include='*.toml' --include='*.lock' \
  --include='*/' --exclude='*' \
  "$FMM_ROOT/src/" "$SCRIPT_DIR/fmm-src/src/"
rsync -a --include='*.rs' --include='*/' --exclude='*' \
  "$FMM_ROOT/benches/" "$SCRIPT_DIR/fmm-src/benches/" 2>/dev/null || true
cp "$FMM_ROOT/docs/fmm-navigate.md" "$SCRIPT_DIR/fmm-src/docs/"
cp "$FMM_ROOT/docs/CLAUDE-SNIPPET.md" "$SCRIPT_DIR/fmm-src/docs/" 2>/dev/null || true
cp "$FMM_ROOT/Cargo.toml" "$FMM_ROOT/Cargo.lock" "$SCRIPT_DIR/fmm-src/"
echo "   Copied $(find "$SCRIPT_DIR/fmm-src" -name '*.rs' | wc -l | tr -d ' ') Rust source files"

# 2. Copy skill file
echo "2. Copying skill file..."
cp "$FMM_ROOT/docs/fmm-navigate.md" "$SCRIPT_DIR/fmm-navigate.md"

# 3. Check .env
echo "3. Checking .env..."
if [[ -f "$SCRIPT_DIR/.env" ]]; then
  source "$SCRIPT_DIR/.env"
  echo "   ANTHROPIC_API_KEY: ${ANTHROPIC_API_KEY:+set (${#ANTHROPIC_API_KEY} chars)}"
  echo "   CODEBASE_PATH: ${CODEBASE_PATH:-NOT SET}"
  if [[ -n "${CODEBASE_PATH:-}" ]]; then
    FILE_COUNT=$(find "$CODEBASE_PATH" -type f \( -name "*.ts" -o -name "*.js" -o -name "*.py" -o -name "*.rs" -o -name "*.go" \) -not -path "*/node_modules/*" -not -path "*/.git/*" 2>/dev/null | wc -l | tr -d ' ')
    echo "   Source files: $FILE_COUNT"
  fi
else
  echo "   .env not found — creating template"
  cat > "$SCRIPT_DIR/.env" <<'EOF'
ANTHROPIC_API_KEY=
CODEBASE_PATH=/path/to/your/codebase
EOF
fi

# 4. Create results dirs
echo "4. Creating results directories..."
mkdir -p "$SCRIPT_DIR/results/A" "$SCRIPT_DIR/results/B" "$SCRIPT_DIR/results/.logs"

# 5. Make scripts executable
echo "5. Making scripts executable..."
chmod +x "$SCRIPT_DIR/run.sh" "$SCRIPT_DIR/entrypoint.sh" "$SCRIPT_DIR/score.py"

echo ""
echo "Setup complete. Next:"
echo "  1. Edit .env with API key and codebase path"
echo "  2. docker compose build"
echo "  3. ./run.sh AB"
echo "  4. python3 score.py"
