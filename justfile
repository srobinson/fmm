default:
    @just --list

# Regenerate docs (templates/SKILL.md, generated_schema.rs, generated_help.rs) from tools.toml.
# Touches tools.toml to force build.rs to re-run without a full recompile.
gen-docs:
    touch tools.toml
    cargo build 2>&1 | grep -vE "^\s*(Compiling|Finished|Running|Fresh)" || true

build:
    cargo build

release:
    cargo build --release

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
