#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# Setup: Prepare fmm source and codebase for Docker-based experiments
# ============================================================================
#
# This script:
#   1. Copies fmm Rust source into fmm-src/ for Docker build context
#   2. Validates .env configuration
#   3. Creates results directories

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

FMM_ROOT="$(cd ../../ && pwd)"

echo "Setting up exp15-isolated experiment environment..."
echo ""

# ─── Copy fmm source for Docker build ───────────────────────────────────────
echo "1. Copying fmm source to fmm-src/..."
rm -rf fmm-src
mkdir -p fmm-src

rsync -a \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'research' \
    --exclude 'examples' \
    --exclude '.nancy' \
    --exclude '.claude' \
    --exclude 'node_modules' \
    "$FMM_ROOT/" fmm-src/

echo "   Copied $(find fmm-src/src -name '*.rs' | wc -l | tr -d ' ') Rust source files"

# ─── Validate .env ───────────────────────────────────────────────────────────
echo ""
echo "2. Checking .env..."
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
    if [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
        echo "   ANTHROPIC_API_KEY: set (${#ANTHROPIC_API_KEY} chars)"
    else
        echo "   WARNING: ANTHROPIC_API_KEY not set in .env"
    fi
    if [[ -n "${CODEBASE_PATH:-}" ]]; then
        if [[ -d "$CODEBASE_PATH" ]]; then
            FILE_COUNT=$(find "$CODEBASE_PATH" -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' 2>/dev/null | head -2000 | wc -l | tr -d ' ')
            echo "   CODEBASE_PATH: $CODEBASE_PATH ($FILE_COUNT source files)"
        else
            echo "   WARNING: CODEBASE_PATH does not exist: $CODEBASE_PATH"
        fi
    else
        echo "   WARNING: CODEBASE_PATH not set in .env"
    fi
else
    echo "   No .env found. Copying .env.example..."
    cp .env.example .env
    echo "   Edit .env with your ANTHROPIC_API_KEY and CODEBASE_PATH"
fi

# ─── Create results directories ─────────────────────────────────────────────
echo ""
echo "3. Creating results directories..."
mkdir -p results/{A,B,C,D}
mkdir -p results/.logs
echo "   Created results/{A,B,C,D} and results/.logs/"

# ─── Make scripts executable ─────────────────────────────────────────────────
echo ""
echo "4. Making scripts executable..."
chmod +x run-isolated.sh entrypoint.sh
echo "   Done."

echo ""
echo "Setup complete. Next steps:"
echo "  1. Edit .env with your API key and codebase path"
echo "  2. docker compose build"
echo "  3. ./run-isolated.sh"
