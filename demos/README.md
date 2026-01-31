# Demo Recordings

Asciinema demo scripts for fmm. Each script can be recorded with:

```bash
asciinema rec --command "./demos/01-getting-started.sh" demos/01-getting-started.cast
```

## Scripts

1. **01-getting-started.sh** — Init, generate, search (30s)
2. **02-navigating.sh** — Search by export, import, dependency, LOC (60s)
3. **03-ai-integration.sh** — MCP tools for LLM navigation (90s)

## Converting to SVG

```bash
# Install svg-term-cli
npm install -g svg-term-cli

# Convert recording to animated SVG
svg-term --in demos/01-getting-started.cast --out docs/src/hero.svg --window --width 80 --height 24
```
