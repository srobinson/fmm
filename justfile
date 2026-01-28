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
