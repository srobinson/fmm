default:
    @just --list

# Regenerate docs (templates/SKILL.md, generated_schema.rs, generated_help.rs) from tools.toml.
# Touches tools.toml to force build.rs to re-run without a full recompile.
gen-docs:
    touch crates/fmm-cli/tools.toml
    cargo build -p fmm 2>&1 | grep -vE "^\s*(Compiling|Finished|Running|Fresh)" || true

build:
    cargo build --workspace

release:
    cargo build --workspace --release

install: release
    cargo install --path crates/fmm-cli

test:
    cargo nextest run --workspace
    cargo test --workspace --doc

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings

check: fmt clippy

ci: check test build
    @echo "All CI checks passed"
