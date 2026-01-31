#!/usr/bin/env bash
# Generate llms-full.txt from all documentation source files.
# Reads SUMMARY.md to determine page order, then concatenates all pages.
set -euo pipefail

DOCS_SRC="$(cd "$(dirname "$0")/src" && pwd)"
OUTPUT="$DOCS_SRC/llms-full.txt"

{
    echo "# fmm — Frontmatter Matters (Full Documentation)"
    echo ""
    echo "> This file contains the complete fmm documentation in a single file,"
    echo "> suitable for ingestion by LLM agents. Generated from source docs."
    echo ""
    echo "---"
    echo ""

    # Concatenate pages in SUMMARY.md order
    for page in \
        introduction.md \
        getting-started/installation.md \
        getting-started/quickstart.md \
        reference/cli.md \
        reference/sidecar-format.md \
        reference/configuration.md \
        reference/mcp-tools.md; do
        filepath="$DOCS_SRC/$page"
        if [ -f "$filepath" ]; then
            cat "$filepath"
            echo ""
            echo "---"
            echo ""
        else
            echo "WARNING: Missing $filepath" >&2
        fi
    done
} > "$OUTPUT"

wc -c < "$OUTPUT" | awk '{printf "✓ llms-full.txt generated (%d bytes)\n", $1}'
