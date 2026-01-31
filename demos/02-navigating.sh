#!/usr/bin/env bash
# Demo: Navigating a Codebase with fmm (60 seconds)
# Record with: asciinema rec --command "./demos/02-navigating.sh" demos/02-navigating.cast
set -e

cd examples/demo-project

echo "# fmm search — structural code navigation"
echo ""
sleep 2

echo "# 1. Find a symbol definition (O(1) lookup):"
echo '$ fmm search --export validateSession'
sleep 1
fmm search --export validateSession
sleep 3

echo ""
echo "# 2. Find all files that import a package:"
echo '$ fmm search --imports express'
sleep 1
fmm search --imports express
sleep 3

echo ""
echo "# 3. Find files that depend on a module:"
echo '$ fmm search --depends-on src/db/client'
sleep 1
fmm search --depends-on src/db/client
sleep 3

echo ""
echo "# 4. Find large files:"
echo '$ fmm search --loc ">25"'
sleep 1
fmm search --loc ">25"
sleep 3

echo ""
echo "# 5. JSON output for programmatic use:"
echo '$ fmm search --export AppError --json'
sleep 1
fmm search --export AppError --json
sleep 3

echo ""
echo "# All lookups use pre-built indexes — no source file scanning."
