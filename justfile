# fmm development commands

# Default recipe: show available commands
default:
    @just --list

# Build the project
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run clippy and check formatting
check:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo fmt --check

# Format code
fmt:
    cargo fmt

# Run the CLI (pass args after --)
run *ARGS:
    cargo run -- {{ARGS}}

# Clean build artifacts
clean:
    cargo clean

# Full CI check: format, clippy, test, build
ci: check test build
    @echo "✓ All CI checks passed"

# Generate manifest for examples
example-generate:
    cargo run -- generate examples/

# Validate examples
example-validate:
    cargo run -- validate examples/

# Generate CLI reference docs from clap definitions
generate-cli-docs: build
    ./target/debug/fmm --markdown-help > docs/src/reference/cli.md
    @echo "✓ docs/src/reference/cli.md updated"

# Generate man pages
generate-man-pages: build
    ./target/debug/fmm --generate-man-pages docs/man/
    @echo "✓ Man pages written to docs/man/"

# Generate llms-full.txt from doc sources
generate-llms-full:
    ./docs/generate-llms-full.sh

# Generate all docs (CLI reference + llms-full.txt)
generate-docs: generate-cli-docs generate-llms-full

# Check that generated CLI docs are up to date
check-cli-docs: build
    #!/usr/bin/env bash
    set -euo pipefail
    tmp=$(mktemp)
    ./target/debug/fmm --markdown-help > "$tmp"
    if ! diff -q "$tmp" docs/src/reference/cli.md > /dev/null 2>&1; then
        echo "✗ docs/src/reference/cli.md is stale"
        echo "  Run 'just generate-cli-docs' to regenerate"
        rm "$tmp"
        exit 1
    fi
    rm "$tmp"
    echo "✓ CLI docs are up to date"
