# Documentation Generation

All user-facing documentation — MCP tool descriptions, CLI help strings, and the Claude Code skill — derives from a single source of truth: `tools.toml`.

## Single Source of Truth

```
tools.toml
```

Edit here. Never edit the generated files directly — changes will be overwritten on the next build.

## What Gets Generated

| Artefact | Path | Consumer |
|----------|------|----------|
| MCP schema | `src/mcp/generated_schema.rs` | MCP server — what AI agents see for each tool |
| CLI help | `src/cli/generated_help.rs` | `fmm --help` and per-command help text |
| Skill doc | `templates/SKILL.md` | Claude Code skill (pulled by helioy-plugins) |

## How It Works

`build.rs` is a [Cargo build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html) — it runs before compilation on every `cargo build`. It reads `tools.toml`, generates all three artefacts, and writes them to disk.

```
tools.toml  →  build.rs  →  src/mcp/generated_schema.rs
                         →  src/cli/generated_help.rs
                         →  templates/SKILL.md
```

Cargo only re-runs `build.rs` when `tools.toml` or `build.rs` itself changes (via `cargo:rerun-if-changed`). If neither changed, the build script is skipped entirely.

`build.rs` also uses `write_if_changed` — it compares the generated content against what is already on disk and skips the write if identical. This avoids cascading Rust recompilation when the generated `.rs` files are unchanged.

## tools.toml Structure

```toml
# Skill documentation prose and navigation workflow
[skill]
workflow = """
## Navigation Workflow
...
## Navigation Protocol
...
"""

# One entry per tool, in MCP response order
[tools.fmm_lookup_export]
cli_name        = "lookup"               # fmm lookup <name>
mcp_description = "..."                  # shown to AI agents
cli_about       = "..."                  # shown in fmm lookup --help

[[tools.fmm_lookup_export.params]]
name            = "name"
type            = "string"
required        = true
mcp_description = "..."                  # shown to AI agents
cli_help        = "..."                  # shown in --help
cli_flag        = "name"                 # positional; "--flag" for named flags
```

`mcp_description` and `cli_about`/`cli_help` are independent — they can and often should differ. MCP descriptions are agent-facing prose; CLI help is human-facing and typically shorter.

## What Lives Where

| Content | Location |
|---------|----------|
| Tool descriptions (MCP + CLI) | `tools.toml` `[tools.*]` entries |
| Parameter definitions | `tools.toml` `[[tools.*.params]]` entries |
| Navigation Workflow + quick-reference code blocks | `tools.toml` `[skill] workflow` |
| Navigation Protocol (per-use-case narrative) | `tools.toml` `[skill] workflow` |
| SKILL.md static prose (frontmatter, "Before You Touch Any Code", rules, sidecar) | `build.rs` `fn generate_skill_md()` |
| MCP schema structure | `build.rs` `fn generate_mcp_schema()` |
| CLI help structure | `build.rs` `fn generate_cli_help()` |

## Regenerating Docs

### Fast iteration — docs only

```sh
just gen-docs
```

Touches `tools.toml` to force the build script to re-run, then runs a doc-only cargo build. No tree-sitter, no full recompile — only `build.rs` executes and the generated `.rs` files are updated if needed.

### Full build

```sh
just build
```

Regenerates docs as a side effect of the normal build. Use this before committing.

### CI

The CI pipeline runs `cargo build` which regenerates all artefacts. If `templates/SKILL.md` or the generated `.rs` files drift from `tools.toml`, CI will produce a diff — catch this with:

```sh
git diff --exit-code templates/SKILL.md src/mcp/generated_schema.rs src/cli/generated_help.rs
```

## Editing Workflow

### Adding or changing a tool parameter

1. Edit the relevant `[[tools.<name>.params]]` block in `tools.toml`
2. Run `just gen-docs`
3. Verify `templates/SKILL.md` and `src/cli/generated_help.rs` look correct
4. Commit `tools.toml` + all three generated artefacts together

### Updating SKILL.md prose (workflow, navigation protocol)

1. Edit `[skill] workflow` in `tools.toml`
2. Run `just gen-docs`
3. Commit

### Updating SKILL.md structure (frontmatter, rules, sidecar format)

1. Edit `fn generate_skill_md()` in `build.rs`
2. Run `just gen-docs`
3. Commit

### Syncing to helioy-plugins

`templates/SKILL.md` is the canonical skill doc. `helioy-plugins` will eventually pull it directly from fmm releases. Until then, copy manually:

```sh
cp templates/SKILL.md ~/Dev/LLM/DEV/helioy/helioy-plugins/plugins/helioy-tools/skills/fmm/SKILL.md
```

## Future: xtask

Currently `just gen-docs` still runs `cargo build`, which recompiles any changed generated `.rs` files. For true sub-second doc iteration, the generation logic should move to a [cargo xtask](https://github.com/matklad/cargo-xtask): a tiny workspace member that compiles once and runs in milliseconds on subsequent invocations. The xtask would share the TOML parsing structs with `build.rs` via a shared crate, and `build.rs` would become a thin wrapper.
