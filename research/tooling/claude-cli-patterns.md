# Claude CLI Instruction Patterns Research

> Research on what makes Claude Code treat content as instructions vs. ignore it.
> Goal: Determine if `// --- FMM ---` blocks can be treated as actionable instructions.

## Executive Summary

Claude Code has a **hierarchy of instruction sources** with different levels of enforcement:

1. **Hooks** (deterministic, guaranteed execution)
2. **Skills** (on-demand prompt expansion, invoked by description matching)
3. **CLAUDE.md** (advisory, loaded at session start, inconsistently followed)
4. **In-context instructions** (subject to drift and compaction)

**Key insight**: Claude reads instructions but treats them as *advisory* rather than *mandatory*. Task completion takes priority over process compliance. The pattern is: "I read your rules, I understand your rules, I don't follow your rules."

---

## 1. Skills: How Claude Code Skills Work

### Format and Structure

Skills are folders containing a `SKILL.md` file with YAML frontmatter:

```yaml
---
name: skill-name
description: Single-line description with trigger phrases. Use when X, Y, or Z.
---

# Skill Title

[Markdown instructions]
```

**Required fields:**
- `name`: Max 64 chars, lowercase/numbers/hyphens only
- `description`: Max 1024 chars, the **primary trigger mechanism**

**Location hierarchy:**
1. `~/.config/claude/skills/` (user global)
2. `~/.claude/skills/` (user personal)
3. `.claude/skills/` (project-specific)

### Invocation Mechanics

Skills are **invoked based on description matching**, not file content:

1. At startup, Claude loads all skill frontmatter (name + description) into system prompt
2. When user request matches a description, Claude invokes the skill
3. Full `SKILL.md` content is loaded **only after invocation**
4. Scripts/resources are executed on-demand

**Critical insight**: The description is everything. Content in the body labeled "When to Use This Skill" is never seen before invocation decision.

### Invocation Control

```yaml
---
disable-model-invocation: true  # Only user can invoke (good for /deploy, /commit)
user-invocable: false           # Only Claude can invoke (background knowledge)
---
```

### Best Practices for Skills

- **Trigger-rich descriptions**: Include multiple natural language phrases
- **Under 500 lines**: Keeps context manageable
- **15,000 character budget**: Combined skill content limit
- **Gerund naming**: "creating-pdf" not "pdf-creator"
- **Examples over rules**: Show good/bad patterns with code

**Sources:**
- [Extend Claude with skills - Claude Code Docs](https://code.claude.com/docs/en/skills)
- [Inside Claude Code Skills](https://mikhail.io/2025/10/claude-code-skills/)
- [Agent Skills Deep Dive](https://leehanchung.github.io/blogs/2025/10/26/claude-skills-deep-dive/)

---

## 2. MCP Tools: Discovery and Proactive Usage

### Tool Search Architecture

MCP tools are now **deferred by default** to save context:

1. If MCP tool descriptions exceed 10K tokens, tools are marked `defer_loading: true`
2. Claude sees only a Tool Search Tool, not individual tool definitions
3. When Claude needs a tool, it searches by keywords
4. 3-5 relevant tools (~3K tokens) are loaded per query

**Result**: 85% token reduction while maintaining tool access.

### Making Claude Use Tools Proactively

Tool usage depends on **description quality**:

```json
{
  "name": "generate_frontmatter",
  "description": "Generate TypeScript frontmatter types from schema. Use when user asks about types, schemas, frontmatter, or FMM configuration."
}
```

**Tips for better discovery:**
- Keyword-rich descriptions
- Multiple use-case phrases
- Concrete examples of when to use

### Configuration

Tools configured in `~/.claude/settings.json` or `.claude/settings.json`:

```json
{
  "mcpServers": {
    "my-tool": {
      "command": "node",
      "args": ["./mcp-server.js"]
    }
  }
}
```

**Sources:**
- [Connect Claude Code to tools via MCP](https://code.claude.com/docs/en/mcp)
- [MCP Tool Search Guide](https://www.atcyrus.com/stories/mcp-tool-search-claude-code-context-pollution-guide)

---

## 3. CLAUDE.md: Patterns Followed vs. Ignored

### What Gets Followed

- **Build commands**: `npm run build`, `cargo test` - concrete, actionable
- **File path references**: "See `src/config.ts` for settings"
- **Explicit "do not" rules**: Work better when providing alternatives
- **Short, clear instructions**: Under 150-200 instructions total

### What Gets Ignored

- **Style guidelines**: Use linters instead (LLMs are slow/expensive for this)
- **Process checklists**: "Always update CHANGELOG" - drift toward task completion
- **Conditional instructions**: Complex if/then logic
- **Verbose explanations**: Important rules get lost in noise

### The Instruction Drift Problem

**Pattern observed:**
1. Rule exists in CLAUDE.md
2. Claude acknowledges the rule
3. Claude completes task without following rule
4. User reminds Claude
5. Claude apologizes
6. Next session: repeat from step 3

**Root cause**: Claude prioritizes task completion over process compliance. Instructions are advisory, not mandatory.

### Best Practices

```markdown
# Project: FMM

## Commands
- Build: `cargo build`
- Test: `cargo test`

## Key Files
- Config: `src/config/mod.rs`
- CLI: `src/cli/mod.rs`

## Rules
- Never commit to main directly
- All new code needs tests
```

**What works:**
- Keep under 150 instructions
- Be concise, not comprehensive
- Use progressive disclosure: point to files, don't embed content
- Put documents at TOP of prompts (before queries)

### Proposed Enforcement Syntax (Feature Request)

```markdown
<!-- ENFORCE -->
- Always add TSDoc comments on fixes
- Always update CHANGELOG.md
<!-- /ENFORCE -->
```

Not yet implemented, but under discussion.

**Sources:**
- [Writing a good CLAUDE.md](https://www.humanlayer.dev/blog/writing-a-good-claude-md)
- [The Complete Guide to CLAUDE.md](https://www.builder.io/blog/claude-md-guide)
- [CLAUDE.md Best Practices](https://arize.com/blog/claude-md-best-practices-learned-from-optimizing-claude-code-with-prompt-learning/)
- [GitHub Issue #18660 - Instructions not followed](https://github.com/anthropics/claude-code/issues/18660)

---

## 4. System Prompt Instruction Formats

### XML Tags for Structure

Claude responds well to XML-structured prompts:

```xml
<instructions>
1. Parse the input file
2. Generate TypeScript types
3. Write to output location
</instructions>

<context>
This is the FMM frontmatter generator.
</context>

<example>
Input: { "title": "string" }
Output: export interface Frontmatter { title: string; }
</example>
```

**Recommended tags:**
- `<instructions>` - What to do
- `<context>` - Background info
- `<example>` - Input/output pairs
- `<constraints>` - Limitations
- `<thinking>` / `<answer>` - For chain of thought

### Priority Hierarchy

Claude interprets XML tags as **structured priority boundaries**:
- Outer tags (`<task>`, `<context>`) establish high-level intent
- Nested tags (`<reasoning>`, `<constraints>`) provide execution details

### Action vs. Information Modes

```xml
<default_to_action>
Implement changes rather than only suggesting them.
If intent is unclear, infer the most useful action and proceed.
</default_to_action>
```

Or conversely:

```xml
<do_not_act_before_instructions>
Default to providing information and recommendations.
Do not modify files unless clearly instructed.
</do_not_act_before_instructions>
```

**Sources:**
- [Use XML tags to structure prompts](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/use-xml-tags)
- [Prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-4-best-practices)

---

## 5. Comment Patterns and Special Syntax

### Custom Comment Directives

Developers have created custom comment directive systems:

```typescript
// @implement
// Create a caching layer for API responses
// Cache duration: 5 minutes
// Invalidation: on user logout

export function fetchUser() {
  // Claude will implement and convert to JSDoc
}
```

**How it works:**
1. Define directive patterns in `~/.claude/CLAUDE.md`
2. Claude recognizes patterns when reading files
3. Acts on instructions, converts to documentation

### @docs Directive

```typescript
// @docs https://api.example.com/v2/spec
// (Claude will fetch and reference this documentation)
```

**Security note**: Requires prompt injection checks on external URLs.

### Magic Words / Extended Thinking

| Phrase | Token Budget |
|--------|--------------|
| "think" | 4,000 |
| "think hard" / "think deeply" / "megathink" | 10,000 |
| "think harder" / "ultrathink" | 31,999 |

**Note**: As of late 2025, ultrathink is deprecated - 31,999 tokens is now the default budget.

### File Reference Syntax

```
@path/to/file.ts
```

Resolves path, reads file, adds to context.

**Sources:**
- [Comment Directives for Claude Code](https://giuseppegurgone.com/comment-directives-claude-code)
- [What is UltraThink](https://claudelog.com/faqs/what-is-ultrathink/)

---

## 6. Hooks: Deterministic Behavior Enforcement

### Why Hooks Matter

Hooks are **the only guaranteed enforcement mechanism**. Unlike CLAUDE.md (advisory), hooks execute deterministically.

### Hook Events

| Event | When | Can Block? |
|-------|------|------------|
| PreToolUse | Before tool calls | Yes |
| PostToolUse | After tool completion | No |
| UserPromptSubmit | When user submits | Yes |
| PermissionRequest | When Claude requests permission | Yes |
| Stop | When agent finishes | No |
| SessionEnd | Session terminates | No |

### Configuration

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "./validate-edit.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "prettier --write ${file}"
          }
        ]
      }
    ]
  }
}
```

### Exit Codes

- `0`: Continue normally
- `2`: Block action, feed stderr to Claude as error
- JSON with `"permissionDecision": "deny"`: Block with message

### Input Modification (v2.0.10+)

PreToolUse hooks can modify tool inputs before execution:

```bash
#!/bin/bash
# Receives JSON via stdin, outputs modified JSON
jq '.parameters.content += "\n// Auto-generated"'
```

**Sources:**
- [Get started with Claude Code hooks](https://code.claude.com/docs/en/hooks-guide)
- [Complete guide to hooks](https://www.eesel.ai/blog/hooks-in-claude-code)

---

## 7. Key Question: Can `// --- FMM ---` Blocks Be Instructions?

### Current Reality

No, not natively. Claude reads code comments as **context**, not **instructions**. Comments inform understanding but don't trigger action.

### Strategies to Make FMM Blocks Actionable

#### Option 1: CLAUDE.md Directive Pattern

Add to project's `.claude/CLAUDE.md`:

```markdown
## FMM Frontmatter Blocks

When you encounter comments in the format:
\`\`\`
// --- FMM ---
// field: type
// --- END FMM ---
\`\`\`

These are frontmatter type definitions. You should:
1. Parse the field definitions
2. Run `fmm generate` to update TypeScript types
3. Ensure generated types match the comment spec
```

**Limitation**: Advisory only, may drift.

#### Option 2: Custom Skill

Create `.claude/skills/fmm/SKILL.md`:

```yaml
---
name: fmm-frontmatter
description: Process FMM frontmatter blocks. Use when seeing // --- FMM --- comments, discussing frontmatter types, or when user mentions fmm generate.
---

# FMM Frontmatter Processing

When you see `// --- FMM ---` blocks in code:

1. Parse field definitions between markers
2. Run `fmm generate` to create TypeScript types
3. Validate generated types match the spec

## Example

Input:
\`\`\`typescript
// --- FMM ---
// title: string
// date: Date
// draft?: boolean
// --- END FMM ---
\`\`\`

Output:
\`\`\`typescript
export interface Frontmatter {
  title: string;
  date: Date;
  draft?: boolean;
}
\`\`\`
```

**Advantage**: On-demand loading, trigger-rich description.

#### Option 3: Hook Enforcement

Create PostToolUse hook to auto-run fmm when files with markers are edited:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/check-fmm-blocks.sh"
          }
        ]
      }
    ]
  }
}
```

```bash
#!/bin/bash
# hooks/check-fmm-blocks.sh
FILE=$(echo "$HOOK_INPUT" | jq -r '.parameters.file_path // .parameters.path')
if grep -q "// --- FMM ---" "$FILE" 2>/dev/null; then
  fmm generate
fi
```

**Advantage**: Deterministic, guaranteed execution.

#### Option 4: MCP Tool Integration

Create an MCP server that:
1. Watches for FMM blocks in files
2. Provides a `process_fmm_block` tool
3. Claude discovers via tool search when relevant

---

## 8. Recommendations for FMM Integration

### Immediate Actions

1. **Create FMM skill** in `.claude/skills/fmm/SKILL.md`
   - Rich description with trigger phrases
   - Include examples of block format and expected output
   - Reference the fmm CLI commands

2. **Add to project CLAUDE.md**:
   ```markdown
   ## FMM Frontmatter

   This project uses FMM for frontmatter type generation.
   See `// --- FMM ---` blocks in source files.
   Run `fmm generate` after modifying frontmatter schemas.
   ```

3. **Consider a PostToolUse hook** for guaranteed regeneration

### Format Recommendations

For the FMM block format itself, consider:

```typescript
// --- FMM ---
// @schema: BlogPost
// title: string (required)
// date: Date (required)
// tags: string[] (default: [])
// draft: boolean (default: false)
// --- END FMM ---
```

Adding `@schema:` gives Claude a clear actionable anchor.

### Long-term: MCP Integration

For full integration:
1. Create `fmm-mcp` server
2. Tools: `parse_fmm_block`, `generate_types`, `validate_schema`
3. Register in settings.json
4. Claude discovers and uses proactively

---

## Summary: What Triggers Claude to Pay Attention?

| Pattern | Attention Level | Enforcement |
|---------|-----------------|-------------|
| Hooks | Guaranteed | Deterministic |
| Skill description match | High | On-demand |
| XML tags in prompt | High | In-context |
| CLAUDE.md rules | Medium | Advisory |
| Comments in code | Low | Contextual only |
| Body of SKILL.md | Only post-invocation | - |

**Magic words that help:**
- "IMPORTANT:", "CRITICAL:" - mild emphasis (less effective on Opus 4.5)
- "You MUST", "Always", "Never" - directive language
- "think", "think hard", "ultrathink" - extended reasoning
- Explicit XML tags - structural clarity

**What doesn't work:**
- Verbose explanations (get ignored)
- Rules buried in long documents
- Negative-only constraints without alternatives
- Process checklists (task completion wins)

---

## References

### Official Documentation
- [Claude Code Skills](https://code.claude.com/docs/en/skills)
- [Claude Code Hooks](https://code.claude.com/docs/en/hooks-guide)
- [Claude Code MCP](https://code.claude.com/docs/en/mcp)
- [Claude Code Best Practices](https://code.claude.com/docs/en/best-practices)

### Community Resources
- [GitHub: anthropics/skills](https://github.com/anthropics/skills)
- [GitHub: claude-code-system-prompts](https://github.com/Piebald-AI/claude-code-system-prompts)
- [CLAUDE.md Complete Guide](https://www.builder.io/blog/claude-md-guide)
- [Comment Directives](https://giuseppegurgone.com/comment-directives-claude-code)

### Issue Discussions
- [Instructions not followed #18660](https://github.com/anthropics/claude-code/issues/18660)
- [Ignores CLAUDE.md during multi-step #18454](https://github.com/anthropics/claude-code/issues/18454)

---

*Research compiled: 2026-01-28*
