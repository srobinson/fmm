#!/usr/bin/env bash
# Demo: Getting Started with fmm (30 seconds)
# Record with: asciinema rec --command "./demos/01-getting-started.sh" demos/01-getting-started.cast
set -e

demo_dir=$(mktemp -d)
cp -r examples/demo-project/src "$demo_dir/"
# Remove pre-generated sidecars so we can show generation
find "$demo_dir" -name "*.fmm" -delete
cd "$demo_dir"

echo "# Welcome to fmm — Frontmatter Matters"
echo "# Let's set up a project in 30 seconds"
echo ""
sleep 2

echo '$ fmm init --no-generate'
sleep 1
fmm init --no-generate
sleep 2

echo ""
echo '$ fmm generate'
sleep 1
fmm generate
sleep 2

echo ""
echo "# Now let's find where createSession is defined:"
echo ""
echo '$ fmm search --export createSession'
sleep 1
fmm search --export createSession
sleep 3

echo ""
echo "# That's it — metadata-first code navigation."

cd /
rm -rf "$demo_dir"
