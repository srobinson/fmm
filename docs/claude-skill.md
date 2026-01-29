# fmm Claude Skill

Install the skill automatically:

```bash
fmm init --skill
```

This installs `.claude/skills/fmm-navigate.md` which teaches Claude Code to:
- Check `.fmm/index.json` before reading source files
- Use the export index for O(1) symbol lookups
- Use dependency graphs for impact analysis
- Fall back to standard exploration if no manifest

The skill file source is at `docs/fmm-navigate.md`.

For MCP server integration (recommended alongside the skill):

```bash
fmm init --mcp
```

Or install everything at once:

```bash
fmm init --all
```
