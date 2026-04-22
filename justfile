default:
    @just --list

FMM_LOCAL_BIN := env_var_or_default("FMM_LOCAL_BIN", "/Users/alphab/.cargo/bin/fmm")

# Regenerate docs (templates/SKILL.md, generated_schema.rs, generated_help.rs) from tools.toml.
# Touches tools.toml to force build.rs to re-run without a full recompile.
gen-docs:
    touch crates/fmm-cli/tools.toml
    cargo build -p fmm 2>&1 | grep -vE "^\s*(Compiling|Finished|Running|Fresh)" || true

build:
    cargo build --workspace

build-local:
    FMM_GIT_SHA="$(git rev-parse --short=7 HEAD)" cargo build --release -p fmm

release:
    cargo build --workspace --release

install: release
    cargo install --path crates/fmm-cli --force

install-local: build-local
    @set -eu; \
    src="$(pwd)/target/release/fmm"; \
    dest="{{FMM_LOCAL_BIN}}"; \
    case "$dest" in /*) ;; *) dest="$(pwd)/$dest";; esac; \
    if [ "$src" = "$dest" ]; then \
        echo "Built $src"; \
    else \
        mkdir -p "$(dirname "$dest")"; \
        install -m 755 "$src" "$dest"; \
        echo "Installed $dest"; \
    fi

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
