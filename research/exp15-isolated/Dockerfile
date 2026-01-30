# fmm experiment base image — pristine environment with zero ambient config
#
# Multi-stage build:
#   1. Build fmm from source (Rust stage)
#   2. Runtime with Node 22 + Claude CLI + fmm binary
#
# Usage:
#   docker build -t fmm-exp .
#   docker run -e ANTHROPIC_API_KEY=sk-... fmm-exp <condition> <task_idx> <run_num>

# ─── Stage 1: Build fmm from Rust source ─────────────────────────────────────
FROM rust:1.93-bookworm AS builder

WORKDIR /build
COPY fmm-src/ .
RUN cargo build --release && \
    strip target/release/fmm

# ─── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM node:22-bookworm-slim

# System deps for Claude CLI
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        rsync \
        python3 \
    && rm -rf /var/lib/apt/lists/*

# Install Claude CLI (npm global)
RUN npm install -g @anthropic-ai/claude-code && \
    npm cache clean --force

# Copy fmm binary from builder
COPY --from=builder /build/target/release/fmm /usr/local/bin/fmm

# Ensure NO ambient config exists
# - No ~/.claude/ directory
# - No ~/.config/ Claude config
# - No global CLAUDE.md
# - No skills, no MCP servers
RUN rm -rf /root/.claude /root/.config /root/.local && \
    mkdir -p /root && \
    echo '{}' > /root/.claude.json 2>/dev/null || true

# Working directory for experiments
WORKDIR /experiment

# Entrypoint script
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# API key must be passed at runtime — never baked in
ENV ANTHROPIC_API_KEY=""

ENTRYPOINT ["entrypoint.sh"]
