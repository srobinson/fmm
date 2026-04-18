# fmm (frontmatter-matters)

Structural intelligence for codebases. Parses source files with tree-sitter, extracts exports/imports/dependencies, stores them in a SQLite index, and exposes the results through a CLI and MCP server. AI agents use this to orient in a codebase before reading source files.

## Architecture

Rust workspace with three crates:

```
crates/
  fmm-core/    # Domain logic: parsers, manifest, search, config, formatting
  fmm-cli/     # CLI binary + built-in MCP server (JSON-RPC over stdio)
  fmm-store/   # Persistence layer: SQLite read/write via rusqlite
```

**fmm-core** (21.9k LOC) contains the parser registry, 18 language parsers (tree-sitter based), the in-memory manifest, search/filter engine, cross-package resolver (oxc_resolver), and configuration loading. The parser module alone accounts for 12.5k LOC across language implementations.

**fmm-cli** (18.4k LOC) provides the user-facing CLI (clap) and an MCP server with 8 tools. CLI commands and MCP tools share the same core logic. Tool schemas and help text are code-generated from `tools.toml` at build time.

**fmm-store** (2.6k LOC) owns the SQLite schema and implements read/write operations against `.fmm.db`. Provides both `SqliteStore` and `InMemoryStore` (for testing).

### Data flow

```
Source files  ->  tree-sitter parse  ->  export/import extraction  ->  .fmm.db (SQLite)
                                                                          |
CLI queries  <--------------------------------------------------------------+
MCP tools    <--------------------------------------------------------------+
```

Indexing is incremental (mtime-based). Only changed files are re-parsed. The index loads into memory in milliseconds for query operations.

## Supported languages

TypeScript/JavaScript (most mature), Python, Rust, Go, Java, C, C++, C#, Ruby, PHP, Swift, Kotlin, Dart, Elixir, Lua, Scala, Zig.

Each parser extracts exports, imports, dependencies, and LOC. Some languages include extra metadata (e.g., Rust: derives, unsafe blocks, trait impls; Python: decorators).

## Key dependencies

| Dependency | Role |
|---|---|
| tree-sitter + 18 grammar crates | Source parsing |
| rusqlite (bundled) | SQLite persistence |
| clap | CLI argument parsing |
| rayon | Parallel file processing |
| oxc_resolver | Cross-package import resolution |
| serde / serde_json | Serialization |
| notify | File system watching |
| ignore | .gitignore-aware file walking |

## Build, test, run

Requires Rust 2024 edition (resolver v3). Use `just` for all workflows:

```bash
just build     # cargo build --workspace
just test      # cargo nextest run + doctests
just check     # cargo fmt + clippy
just ci        # check + test + build
just install   # release build + cargo install
```

**Do not run `cargo test` directly.** Config tests mutate environment variables and require the process-per-test isolation that only nextest provides.

### Distribution

Published to npm as `frontmatter-matters`. The `npm/` directory contains a thin wrapper that downloads the platform-specific binary from GitHub releases.

```bash
npx frontmatter-matters init    # one-command setup
```

## CI/CD

Three GitHub Actions workflows:
- `ci.yml` runs on PRs and pushes to main
- `release.yml` builds platform binaries and publishes to npm
- `docs.yml` deploys documentation site

## Current status

Version **0.2.3**. Actively developed. Core indexing pipeline is stable. MCP server and CLI are production-ready for TypeScript/Python/Rust codebases. Other language parsers exist but have less validation.

Part of the [Helioy](https://github.com/srobinson) ecosystem.
