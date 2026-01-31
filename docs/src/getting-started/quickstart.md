# Quickstart

Get your first sidecar in 60 seconds.

## 1. Initialize

```bash
cd your-project
fmm init
```

This creates config files and generates sidecars in one step. You'll see output like:

```
Frontmatter Matters — metadata sidecars for LLM code navigation

✓ Created .fmmrc.json with default configuration
✓ Installed Claude skill at .claude/skills/fmm-navigate.md
✓ Created .mcp.json with fmm server configuration

✓ 247 source files detected (ts, py, rs)
Generating sidecars...
Found 247 files to process
✓ src/auth.ts.fmm
✓ src/db.ts.fmm
...
✓ 247 sidecar(s) written

Sample sidecar for src/auth.ts:
  ---
  file: src/auth.ts
  exports: [authenticate, createSession, validateToken]
  imports: [jsonwebtoken, bcrypt]
  dependencies: [./db, ./config]
  loc: 142
  ---

  next: Try: fmm search --export authenticate

Setup complete!
  Config:   .fmmrc.json
  Skill:    .claude/skills/fmm-navigate.md
  MCP:      .mcp.json

  ✓ Your AI assistant now navigates this codebase via metadata sidecars
```

## 2. Search

Find where a symbol is defined:

```bash
fmm search --export createStore
```

```
✓ 1 file(s) found:

src/store/index.ts
  exports: createStore, configureStore, StoreConfig
  imports: redux, immer
  loc: 89
```

## 3. Validate in CI

Add to your CI pipeline to ensure sidecars stay current:

```bash
fmm validate
```

Returns exit code 0 if all sidecars match their source files, 1 if any are stale.

## What's in a sidecar?

Each `.fmm` file contains:

- **file** — relative path to the source file
- **exports** — public symbols (functions, classes, types, constants)
- **imports** — external package imports
- **dependencies** — relative file dependencies within the project
- **loc** — line count

This is enough for an LLM to navigate your entire codebase without reading source files.

## Next steps

- [CLI Reference](../reference/cli.md) — all commands and options
- [Sidecar Format](../reference/sidecar-format.md) — full YAML specification
- [Configuration](../reference/configuration.md) — `.fmmrc.json` options
