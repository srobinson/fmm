#!/usr/bin/env bash
# Generate llms-full.txt by parsing SUMMARY.md for page order, then concatenating all pages.
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

    # Extract page paths from SUMMARY.md links: - [Title](path.md)
    mapfile -t pages < <(sed -n 's/.*](\([^)]*\)).*/\1/p' "$DOCS_SRC/SUMMARY.md")

    for page in "${pages[@]}"; do
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
