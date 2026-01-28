# Claude Code Instruction Injection Methods

Research into programmatically providing custom instructions to Claude Code agents without modifying `~/.claude/CLAUDE.md`.

**Goal**: Run Claude Code on a repository with custom instructions injected, without touching user config.

---

## Summary Table

| Method | Works for Fresh Clones | Programmatic | Persists in Repo | Limitations |
|--------|----------------------|--------------|------------------|-------------|
| Project CLAUDE.md | Yes | Yes | Yes | Must be committed |
| CLI --append-system-prompt | Yes | Yes | No | One-shot only, not interactive |
| CLI --system-prompt-file | Yes | Yes | No | One-shot only, not interactive |
| SessionStart Hook | Yes | Yes | Yes | Known bug for brand new sessions |
| UserPromptSubmit Hook | Yes | Yes | Yes | Fires on every prompt |
| MCP Server Instructions | Yes | Yes | Yes | Requires MCP setup |
| Skills (SKILL.md) | Yes | Partial | Yes | Model decides when to invoke |
| Environment Variables | Yes | Yes | No | Limited instruction capacity |
| .claude/settings.json | Yes | Yes | Yes | For config, not instructions |

---

## 1. Project-Level CLAUDE.md

**Works for fresh clones: YES**

CLAUDE.md files placed in the repository are automatically loaded when Claude Code runs from that directory.

### Locations (in order of loading)

1. `./CLAUDE.md` - Repository root (most common)
2. Parent directories - Useful for monorepos
3. Child directories - Loaded on demand when entering subdirectories
4. `.claude/CLAUDE.md` - Alternative location in .claude folder

### How It Works

- Claude Code scans for CLAUDE.md files at session start
- Content is injected as a `<system-reminder>` in user messages
- Instructions include: "IMPORTANT: this context may or may not be relevant to your tasks"

### Best Practices

- Keep focused and relevant - overly broad instructions get ignored
- Refine like any frequently-used prompt
- Use emphasis ("IMPORTANT", "YOU MUST") for critical rules

### Pros/Cons

**Pros:**
- Works automatically on fresh clones
- Version controlled with the repo
- Team members get the same instructions

**Cons:**
- Instructions are visible in repo
- Must be committed to work for others
- Large files may be partially ignored

### Sources
- [Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Writing a Good CLAUDE.md](https://www.humanlayer.dev/blog/writing-a-good-claude-md)

---

## 2. CLI Flags for System Prompts

**Works for fresh clones: YES (but one-shot only)**

### Available Flags

| Flag | Description | Use Case |
|------|-------------|----------|
| `--system-prompt "..."` | Replace entire system prompt | Complete control |
| `--system-prompt-file path` | Load system prompt from file | Version-controlled prompts |
| `--append-system-prompt "..."` | Add to default system prompt | Extend capabilities |
| `--append-system-prompt-file path` | Append file to system prompt | Team-shared additions |

### Usage Examples

```bash
# One-shot with custom prompt appended
claude -p "Fix the bug in main.py" --append-system-prompt "Always suggest tests"

# Load instructions from file
claude -p "Review this code" --system-prompt-file ./instructions.txt

# Replace entire system prompt (removes Claude Code capabilities!)
claude -p "Analyze this" --system-prompt "You are a security reviewer"
```

### Critical Limitations

- **One-shot only**: These flags only work with `-p` (non-interactive mode)
- **No interactive sessions**: Cannot use in REPL mode
- **No session persistence**: Each `-p` command is a separate execution
- `--system-prompt` removes all default Claude Code instructions

### Recommended Approach

Use `--append-system-prompt` or `--append-system-prompt-file` to preserve Claude Code's built-in capabilities while adding custom requirements.

### Sources
- [Claude Code CLI Reference](https://code.claude.com/docs/en/cli-reference)
- [What is --system-prompt-file](https://claudelog.com/faqs/what-is-system-prompt-file-flag-in-claude-code/)

---

## 3. Environment Variables

**Works for fresh clones: YES (must be set externally)**

Claude Code respects numerous environment variables, but none directly inject instructions into the system prompt.

### Relevant Variables for Programmatic Control

```bash
# Model selection
ANTHROPIC_MODEL="claude-sonnet-4-5-20250929"
CLAUDE_CODE_SUBAGENT_MODEL="claude-haiku-4-5-20250929"

# Behavior modification
CLAUDE_CODE_ACTION="plan"  # Values: acceptEdits, plan, bypassPermissions, default
CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR="1"

# Environment setup (sourced before Bash commands)
CLAUDE_ENV_FILE="/path/to/env-setup.sh"

# Configuration directory override
CLAUDE_CONFIG_DIR="/custom/config/path"

# Project context
CLAUDE_PROJECT_DIR="/path/to/project"  # Available to hooks
```

### CLAUDE_CODE_ACTION Values

| Value | Behavior |
|-------|----------|
| `default` | Standard interactive behavior |
| `acceptEdits` | Ask before file changes |
| `plan` | Analysis only, no modifications |
| `bypassPermissions` | Auto-accept all operations (dangerous) |

### Indirect Instruction Injection

While env vars cannot directly inject instructions, they can:
1. Set `CLAUDE_ENV_FILE` to source environment context
2. Configure behavior modes that affect how Claude operates
3. Pass data to hooks which CAN inject instructions

### Sources
- [Claude Code Environment Variables](https://gist.github.com/unkn0wncode/f87295d055dd0f0e8082358a0b5cc467)
- [Environment Variables Guide](https://medium.com/@dan.avila7/claude-code-environment-variables-a-complete-reference-guide-41229ef18120)

---

## 4. MCP Server Instructions

**Works for fresh clones: YES (if MCP servers configured in repo)**

MCP servers can provide instructions through the `server instructions` field, which helps Claude understand when and how to use the server's tools.

### How It Works

1. Server defines an `instructions` field in its configuration
2. When Tool Search is enabled, these instructions guide Claude
3. Tool descriptions also serve as per-tool instructions

### Configuration in .claude/settings.json

```json
{
  "mcpServers": {
    "my-server": {
      "command": "node",
      "args": ["/path/to/server.js"],
      "instructions": "Use this server for database operations. Always validate inputs before executing queries."
    }
  }
}
```

### Tool Description as Instructions

When building an MCP server, tool descriptions serve as instructions:

```typescript
server.tool(
  "execute_query",
  "Execute a SQL query. IMPORTANT: Always use parameterized queries to prevent injection.",
  { query: z.string(), params: z.array(z.any()).optional() }
);
```

### Limitations

- Requires MCP server setup
- Instructions are tool-focused, not general purpose
- Tool Search threshold (10% context) affects when loaded

### Sources
- [Claude Code MCP Documentation](https://code.claude.com/docs/en/mcp)

---

## 5. Skills Auto-Invocation

**Works for fresh clones: YES (but model decides when to invoke)**

Skills are modular capabilities that Claude can invoke autonomously based on context.

### How Skills Work

1. At session start, skill descriptions (30-50 tokens each) load into context
2. Claude reads the task and decides which skills are relevant
3. Full skill content loads only when invoked

### Skill Structure (.claude/skills/my-skill/SKILL.md)

```yaml
---
name: my-skill
description: Provides coding standards and review guidelines
disable-model-invocation: false  # Set true to prevent auto-invoke
---

# My Skill Instructions

When reviewing code:
1. Check for security vulnerabilities
2. Ensure proper error handling
3. Verify test coverage
```

### Auto-Invocation Challenges

Multiple users report skills don't reliably auto-activate. Solutions:

1. **Use hooks to force invocation**: A UserPromptSubmit hook can detect trigger words and tell Claude to use specific skills
2. **Clear, specific descriptions**: Better descriptions improve activation rates
3. **Testing shows 80-84% success** with optimized descriptions vs 50% baseline

### Controlling Invocation

```yaml
---
name: restricted-skill
description: Internal admin operations
disable-model-invocation: true  # Only manual /restricted-skill works
---
```

### Subagent Preloading

Skills can be preloaded for subagents:
```json
{
  "preloadSkills": ["my-skill"]
}
```

This injects full skill content at subagent startup.

### Sources
- [Claude Code Skills Documentation](https://code.claude.com/docs/en/skills)
- [Skills Don't Auto-Activate (Workaround)](https://scottspence.com/posts/claude-code-skills-dont-auto-activate)
- [Claude Agent Skills Deep Dive](https://leehanchung.github.io/blogs/2025/10/26/claude-skills-deep-dive/)

---

## 6. Hooks That Inject Context

**Works for fresh clones: YES (most reliable method)**

Hooks provide deterministic control over Claude Code behavior, including instruction injection.

### Two Key Hooks for Instruction Injection

#### SessionStart Hook

Fires when Claude Code starts a new session.

```json
// .claude/settings.json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "cat ./project-instructions.txt"
      }]
    }]
  }
}
```

**Known Bug**: SessionStart hooks execute but output may not be injected for brand new sessions. Works correctly for `/clear`, `/compact`, and URL resume operations.

#### UserPromptSubmit Hook (RECOMMENDED)

Fires on every user prompt submission. Most reliable for instruction injection.

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "echo 'IMPORTANT: Follow the project coding standards in STANDARDS.md'"
      }]
    }]
  }
}
```

### Context Injection Methods

#### Method 1: Plain Text stdout (Simple)

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "echo 'Always write tests for new functions'"
      }]
    }]
  }
}
```

Any text on stdout is added as context.

#### Method 2: JSON with additionalContext (Structured)

```bash
#!/bin/bash
# .claude/hooks/inject-context.sh
cat << 'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "Project Rules:\n1. Use TypeScript strict mode\n2. All functions must have JSDoc\n3. Run tests before committing"
  }
}
EOF
```

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "./.claude/hooks/inject-context.sh"
      }]
    }]
  }
}
```

### Exit Codes

| Code | Behavior |
|------|----------|
| 0 | Success - stdout injected as context (UserPromptSubmit/SessionStart) |
| 2 | Blocking error - stderr fed back to Claude |
| Other | Non-blocking error - stderr shown to user |

### Blocking Prompts

```json
{
  "decision": "block",
  "reason": "Cannot proceed: missing required configuration"
}
```

### Dynamic Instruction Loading

```bash
#!/bin/bash
# .claude/hooks/dynamic-context.sh

# Load project-specific instructions
if [ -f "./INSTRUCTIONS.md" ]; then
    cat ./INSTRUCTIONS.md
fi

# Load sprint-specific context
if [ -f "./.sprint-context.md" ]; then
    cat ./.sprint-context.md
fi

# Add git context
echo "Current branch: $(git branch --show-current)"
echo "Recent commits:"
git log --oneline -5
```

### Sources
- [Hooks Reference](https://code.claude.com/docs/en/hooks)
- [How to Configure Hooks](https://claude.com/blog/how-to-configure-hooks)
- [Claude Code Hooks Mastery](https://github.com/disler/claude-code-hooks-mastery)

---

## 7. CLAUDE.local.md and Local Override Files

**Works for fresh clones: NO (gitignored by design)**

### CLAUDE.local.md (DEPRECATED)

Previously allowed personal project-specific preferences that weren't committed. Now deprecated in favor of imports.

### .claude/settings.local.json

For personal settings that shouldn't be committed:

```json
// .claude/settings.local.json (gitignored)
{
  "permissions": {
    "allow": ["Bash(npm run:*)"]
  },
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "echo 'My personal preferences...'"
      }]
    }]
  }
}
```

Claude Code automatically adds this file to .gitignore when created.

### Settings Hierarchy

1. `.claude/settings.local.json` - Personal, gitignored (highest priority)
2. `.claude/settings.json` - Team, version controlled
3. `~/.claude/settings.json` - User-wide defaults (lowest priority)

### Keeping CLAUDE.md Local

To have a personal CLAUDE.md that isn't committed:

```bash
# Add to .git/info/exclude (local to your clone)
CLAUDE.md
```

### Sources
- [Claude Code Settings](https://code.claude.com/docs/en/settings)
- [Keeping CLAUDE.md Out of Shared Repos](https://andyjakubowski.com/engineering/keeping-claude-md-out-of-shared-git-repos)

---

## Recommended Approach for Programmatic Instruction Injection

For running Claude Code on a repo with custom instructions WITHOUT modifying user config:

### Option A: Project Files (Persistent, Version Controlled)

1. Create `CLAUDE.md` in repo root with instructions
2. Create `.claude/settings.json` with hooks configuration
3. Add hook scripts in `.claude/hooks/`

```
my-repo/
  CLAUDE.md                    # Project instructions
  .claude/
    settings.json              # Hooks configuration
    hooks/
      inject-context.sh        # Dynamic context injection
    skills/
      project-rules/
        SKILL.md               # Structured instructions as skill
```

### Option B: CLI Automation (One-Shot, No Repo Changes)

```bash
# Create temporary instruction file
cat > /tmp/instructions.txt << 'EOF'
You are working on the FMM project.
Always follow these rules:
1. Use Rust idioms
2. Add comprehensive error handling
3. Write tests for all public functions
EOF

# Run Claude Code with instructions
claude -p "Implement the feature" --append-system-prompt-file /tmp/instructions.txt
```

### Option C: Wrapper Script (Hybrid)

```bash
#!/bin/bash
# claude-with-instructions.sh

INSTRUCTIONS="
IMPORTANT PROJECT RULES:
- All code must pass clippy with no warnings
- Use structured logging
- Document all public APIs
"

# Run Claude Code with injected instructions
claude -p "$1" --append-system-prompt "$INSTRUCTIONS"
```

Usage: `./claude-with-instructions.sh "Fix the parser bug"`

### Option D: Hook-Based (Best for Interactive Sessions)

```json
// Copy to repo: .claude/settings.json
{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "cat ./.claude/project-instructions.txt 2>/dev/null || true"
      }]
    }]
  }
}
```

```text
# Copy to repo: .claude/project-instructions.txt
PROJECT INSTRUCTIONS:
- This is a Rust CLI tool
- Follow the existing code style
- All changes must include tests
- Run `cargo clippy` before committing
```

---

## Key Takeaways

1. **For fresh clones with version-controlled instructions**: Use project CLAUDE.md + hooks in `.claude/settings.json`

2. **For one-shot automated runs**: Use `--append-system-prompt` or `--append-system-prompt-file`

3. **For interactive sessions with dynamic context**: Use UserPromptSubmit hooks (most reliable)

4. **For tool-specific instructions**: Use MCP server instructions

5. **Avoid**: Modifying `~/.claude/CLAUDE.md` - this is user config and should not be touched programmatically

6. **Known limitation**: SessionStart hooks may not reliably inject context for brand new sessions - use UserPromptSubmit as fallback

---

## References

- [Claude Code Documentation](https://code.claude.com/docs)
- [Claude Code Settings](https://code.claude.com/docs/en/settings)
- [Hooks Reference](https://code.claude.com/docs/en/hooks)
- [CLI Reference](https://code.claude.com/docs/en/cli-reference)
- [MCP Documentation](https://code.claude.com/docs/en/mcp)
- [Skills Documentation](https://code.claude.com/docs/en/skills)
- [Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Claude Code System Prompts (Community)](https://github.com/Piebald-AI/claude-code-system-prompts)
- [Claude Code Hooks Mastery](https://github.com/disler/claude-code-hooks-mastery)
