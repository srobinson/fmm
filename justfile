default:
    @just --list

build:
    cargo build

test:
    cargo test

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings

check: fmt clippy

install:
    cargo install --path .

ci: check test build
    @echo "✓ All CI checks passed"
