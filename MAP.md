<!-- fmm:map sha=805ae4c branch=main dirty=false generated=2026-06-17 files=426 loc=64443 -->
<!-- Stamp note: tracked tree is clean at 805ae4c; only untracked map artifacts (MAP.md, MAP.SKILL.md) are present, so the index byte-corresponds to the commit. -->

# fmm — Codebase MAP

> For an agent that has never seen this repo. Where the load-bearing code is, what the
> boundaries are, what not to break, and where to start. Cite code by `path Symbol`, never line numbers.

## Overview

**fmm (frontmatter-matters)** is code structural intelligence for AI agents: it parses 20+
languages into a queryable index so an agent can answer "who imports what / where is this
symbol / what's the blast radius" in O(1) calls instead of grepping. It is a Rust workspace
of three crates:

- **`fmm-core`** — the library brain: parsing, the in-memory index model, import resolution, search, identity interning, similarity, formatting (161 files, 32.6k LOC).
- **`fmm-cli`** — the `fmm` binary: two front-ends (a clap CLI and an MCP server) over `fmm-core` (72 files, 11.3k LOC).
- **`fmm-store`** — persistence adapters: SQLite + in-memory implementations of the core `FmmStore` port (17 files, 3.8k LOC).

**Start reading at** `crates/fmm-core/src/parser/mod.rs` (the type vocabulary the whole crate imports, ↓82 dependents) and `crates/fmm-core/src/manifest/mod.rs` (`Manifest`, the central index model, ↓79).

## Topology

Indexed: **426 files · 64,443 LOC** (source-only, excluding tests/fixtures: 195 files · 36,123 LOC).

| Top level | Files | LOC | What lives here |
|---|---:|---:|---|
| `crates/` | 402 | 62,753 | The three workspace crates (below) |
| `fixtures/` | 23 | 1,617 | Per-language parser test fixtures (python/, etc.) |
| `npm/` | 1 | 73 | npm distribution shim for the binary |

| `fmm-core/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `parser/` | 71 | 15,000 | Tree-sitter parsers, one per language, behind the `Parser` trait + `ParserRegistry` |
| `manifest/` | 35 | 6,487 | The `Manifest` index model + dependency matching/reverse-index builders |
| `resolver/` | 14 | 3,941 | Import resolution (`ImportResolver`): deno, go, rust, workspace layers |
| `search/` | 21 | 2,330 | Query layer: bare/filter search, dependency-graph queries, cycle detection |
| `format/` | 6 | 1,909 | Output formatters (yaml/list/search) for CLI + MCP |
| `src/` (root) | 5 | 992 | `lib.rs`, `store.rs` (`FmmStore` port), `types.rs`, `error.rs` |
| `config/` | 4 | 766 | Config loading + defaults |
| `identity/` | 1 | 456 | `FileId` / `FileIdentityMap` path↔id interning, `EdgeKind` |
| `graph/` | 2 | 427 | Dependency-graph + cycle primitives |
| `convention/` | 1 | 232 | Naming/convention inference |
| `extractor/` | 1 | 90 | `ParserCache` extraction entry |

| `fmm-cli/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `cli/` | 32 | 5,195 | clap `Commands` enum + per-command `*CommandArgs` structs + handlers |
| `mcp/` | 30 | 4,108 | MCP server: the same queries exposed as agent tools |
| `src/` (root) | 9 | 1,476 | `main.rs`, `read_symbol.rs`, `glossary.rs`, `git.rs`, `cycle_report.rs`, fs/glob utils |
| `read_symbol/` | 1 | 569 | `member_error.rs` member-resolution diagnostics |

| `fmm-store/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `src/` (root) | 7 | 2,342 | `lib.rs`, `sqlite_store.rs`, `writer.rs`, `connection.rs`, `error.rs`, `schema` |
| `reader/` | 6 | 732 | Loading manifest rows back out of SQLite |
| `memory_store/` | 4 | 716 | `InMemoryStore` (tests + non-persistent use) |

## Key components (high fan-in — the spine)

Ranked by direct dependents (`fmm ls --sort-by downstream`). These are the highest-blast-radius edit surfaces.

| Component (`path Symbol`) | ↓ direct | Role / why it is load-bearing |
|---|---:|---|
| `parser/mod.rs` (re-export facade) | 82 | 12-LOC facade that `pub use`s the parser type vocabulary (`Parser`, `ParserRegistry`, `ExportEntry`, `Metadata`, `ParseResult`, `DeclarationKind`, `SymbolVisibility`). Nearly every core file imports parser types through it. Edits are usually additive re-exports; renaming a re-export ripples crate-wide. |
| `manifest/mod.rs Manifest` | 79 | The in-memory index model: `files: HashMap<String,FileEntry>`, `export_index`, `export_locations`, `file_identity`, reverse-deps. The single source of truth every query reads. Changing its shape reaches store, search, resolver, and CLI. |
| `identity/mod.rs FileIdentityMap` | 28 | Path↔`FileId` interning + `EdgeKind {Runtime, TypeOnly}`. Underpins the dependency graph and the type-only-vs-runtime edge distinction (`--edge-mode`). |
| `store.rs FmmStore` | 18 | The persistence **port** (trait) with associated `type Error`. Also defines `GitMeta` + `GIT_*_META_KEY` (the index-meta git stamp data model, PR #154). |
| `parser/builtin/query_helpers.rs` | 20 | Shared tree-sitter query utilities reused by every language parser. |
| `resolver/mod.rs ImportResolver` | 16 | Import-resolution trait + `CrossPackageResolver`; layers deno/go/rust/workspace resolution. |
| `fmm-store memory_store/manifest.rs build_manifest` | 24 | Rebuilds a `Manifest` from stored rows — the read path's core. |
| `search/mod.rs`, `types.rs`, `config/mod.rs` | 11 / 11 / 9 | Query result types, shared core types, config surface. |

## Seams & boundaries

- **Persistence port/adapter (clean inversion).** `FmmStore` (`fmm-core/src/store.rs`) declares the contract — associated `type Error`, `load_manifest`, `load_fingerprints`, `update_file_fingerprint`, batch row writers. Adapters `SqliteStore` and `InMemoryStore` live in `fmm-store` and depend on core's trait; **core never depends on `fmm-store`.** This is the safest place to add a new backend.
- **Parser plug-in boundary.** `Parser` trait (`fmm-core/src/parser/types.rs`: `parse` / `parse_file` / `language_id` / `extensions`, `Send + Sync`). Each language is an impl under `parser/builtin/` (71 files); `ParserRegistry` (`parser/registry.rs`, `factories: HashMap<String, ParserFactory>`) maps language id → factory. Adding a language = new builtin impl + registry entry; no other surface changes.
- **Resolution boundary.** `ImportResolver` trait (`resolver/mod.rs`) with deno/go/rust/workspace layer impls behind `CrossPackageResolver`.
- **Two front-ends, one core.** `fmm-cli/src/cli/` (clap; `Cli` → `Commands` enum → per-command `*CommandArgs`) and `fmm-cli/src/mcp/` (MCP tools) are parallel surfaces over `fmm-core`. Entry: `main.rs main()` → `run()` → `run_command(Commands)`. New behavior should land in core, then be exposed by both front-ends.
- **Dependency cycles.** `fmm cycles` reports ~10 file-level SCCs, but **every one is an intra-module `mod.rs`↔submodule cluster** (idiomatic Rust re-export facades), not architectural debt. fmm itself tags these back-edges `# mod-hierarchy`. The largest clusters to watch if real coupling ever creeps in: `manifest/` (11 files, incl. `dependency_matcher/*` + `store.rs`) and `search/` (8 files). See the friction log: `fmm cycles` cannot currently exclude `# mod-hierarchy` edges.

## Public API surface

- **`fmm-core`** (`lib.rs` public modules): `config, convention, error, extractor, format, graph, identity, manifest, parser, resolver, search, similarity, store, types` + `VERSION`. Headline types: `Manifest`, `FileEntry`, `Parser`, `ParserRegistry`, `FmmStore`, `ImportResolver`, `FileId`, `FmmError`.
- **`fmm-store`** (`lib.rs` re-exports): `SqliteStore`, `InMemoryStore`, `StoreError`, `MemoryStoreError`, `open_db`, `DB_FILENAME`; modules `connection / error / memory_store / sqlite_store / writer / reader`.
- **`fmm-cli`** (binary; the user-facing contract): subcommands `ls, deps, cycles, exports, glossary, outline, lookup, read, search, similar, generate, validate, watch, init, clean, completions, status, sidecar`, plus the matching MCP tool set.

## Patterns & conventions

- **Thin re-export facade modules.** `mod.rs` declares submodules and `pub use`s a curated vocabulary. Cite: `parser/mod.rs` (12 LOC, re-exports 9 types), `fmm-core/src/lib.rs` (17 LOC).
- **Ports as traits, impls elsewhere.** `FmmStore`, `Parser`, `ImportResolver`. `FmmStore` uses an associated `type Error` so each adapter defines its own error type.
- **Layered error strategy.** Typed errors at library boundaries via `thiserror` — `FmmError` (`fmm-core/src/error.rs`: `FileNotFound / ExportNotFound / Config / Parse / Resolve / Store`) and `StoreError` (`fmm-store/src/error.rs`: `Database / NoIndex / VersionMismatch / Migration / Other`) — and `anyhow` at the binary (`fmm-cli/src/main.rs` `run` / `run_command` → `anyhow::Result`).
- **Newtype identity / interning.** `FileId`, `RelativePath`, `FileIdentityMap`. Cite: `identity/mod.rs`.
- **Registry pattern.** `ParserRegistry` (`parser/registry.rs`).
- **CLI arg-struct-per-command.** Each subcommand's args live in `cli/commands/<cmd>.rs` as `<Cmd>CommandArgs` (e.g. `LsCommandArgs`, `DepsCommandArgs`).
- **Test layout.** Inline `#[cfg(test)] mod tests` inside source files (e.g. `resolver/mod.rs` `layer3_*` tests) plus a top-level `fixtures/` tree for parser fixtures. fmm's own `--filter source|tests` is path-based, so inline test blocks do not mark a source file as a test file. Run tests with `just test` (nextest), **not** `cargo test`.

## Health flags (candidates, not verdicts)

- **Heaviest source files** (none exceed the repo's 700-line ceiling; largest 629): `parser/builtin/python/mod.rs` (629), `cli/read_symbol.rs` (608), `similarity.rs` (606), `read_symbol/member_error.rs` (569), `cli/watch.rs` (565), `fmm-store/writer.rs` (563), `format/yaml_formatters.rs` (547), `resolver/deno.rs` (547), `parser/builtin/go.rs` (541), `fmm-store/sqlite_store.rs` (531), `manifest/glossary_builder.rs` (514). Worth a glance as they approach the limit.
- **Cycles:** ~10 SCCs, all mod-hierarchy (see Seams). Not classified as debt.
- **Duplication:** not assessable repo-wide today — `fmm similar` is probe-only (one symbol at a time). Not assessed here; see friction log for the missing `fmm dupes` primitive.

---
*Generated from fmm structural primitives at `805ae4c`. To refresh after a commit: `git diff --name-only 805ae4c..HEAD`, `fmm generate`, then re-run `fmm outline`/`fmm deps` on changed files and patch only the affected sections + re-stamp the header.*
