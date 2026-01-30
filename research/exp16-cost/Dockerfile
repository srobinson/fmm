# exp16: A/B cost experiment
# Multi-stage: build fmm from source, then runtime with Claude CLI

FROM rust:1.93-bookworm AS builder
WORKDIR /build
COPY fmm-src/ .
RUN cargo build --release && strip target/release/fmm

FROM node:22-bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates curl git rsync python3 \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g @anthropic-ai/claude-code && \
    npm cache clean --force

COPY --from=builder /build/target/release/fmm /usr/local/bin/fmm

# Clean slate
RUN rm -rf /root/.claude /root/.config /root/.local && \
    mkdir -p /root && \
    echo '{}' > /root/.claude.json 2>/dev/null || true

WORKDIR /experiment

# Copy skill file for condition B
ARG INCLUDE_FMM_SKILL=false
COPY fmm-navigate.md /usr/local/share/fmm-navigate.md

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENV ANTHROPIC_API_KEY=""

ENTRYPOINT ["entrypoint.sh"]
