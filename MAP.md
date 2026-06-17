<!-- fmm:map sha=29990d0 branch=main dirty=false generated=2026-06-17 files=434 loc=67011 -->

# fmm — Codebase MAP

> For an agent that has never seen this repo. Where the load-bearing code is, what the
> boundaries are, what not to break, and where to start. Cite code by `path Symbol`, never line numbers.

## Overview

**fmm (frontmatter-matters)** is code structural intelligence for AI agents: it parses 20+
languages into a queryable SQLite index so an agent can answer "who imports what / where is this
symbol / what's the blast radius" in O(1) calls instead of grepping. It is a Rust workspace
of three crates:

- **`fmm-core`** — the library brain: parsing, the in-memory index model, import resolution, search, identity interning, similarity, the duplicate scorer, formatting (163 files, 34.0k LOC).
- **`fmm-cli`** — the `fmm` binary: two front-ends (a clap CLI and an MCP server) over `fmm-core` (75 files, 11.9k LOC).
- **`fmm-store`** — persistence adapters: SQLite + in-memory implementations of the core `FmmStore` port (17 files, 3.8k LOC).

**Start reading at** `crates/fmm-core/src/parser/mod.rs` (the type vocabulary the whole crate imports, ↓83 direct dependents) and `crates/fmm-core/src/manifest/mod.rs` (`Manifest`, the central index model, ↓81).

## Topology

Indexed: **434 files · 67,011 LOC** (source-only, excluding tests/fixtures: 200 files · 37,926 LOC).

| Top level | Files | LOC | What lives here |
|---|---:|---:|---|
| `crates/` | 410 | 65,321 | The three workspace crates (below) |
| `fixtures/` | 23 | 1,617 | Per-language parser test fixtures (`sample.rb`, `python/`, …) |
| `npm/` | 1 | 73 | npm distribution shim for the binary |

| `fmm-core/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `parser/` | 71 | 15,156 | Tree-sitter parsers, one per language, behind the `Parser` trait + `ParserRegistry` |
| `manifest/` | 35 | 6,606 | The `Manifest` index model + dependency matching / reverse-index builders + private-member extractors |
| `resolver/` | 15 | 3,952 | Import resolution (`ImportResolver`): deno, go, rust, workspace layers |
| `search/` | 21 | 2,537 | Query layer: bare/filter search, dependency-graph queries, cycle detection |
| `format/` | 6 | 2,019 | Output formatters (yaml/list/search) for CLI + MCP |
| `src/` (root) | 6 | 1,649 | `lib.rs`, `store.rs` (`FmmStore` port), `types.rs`, `error.rs`, `similarity.rs`, `dupes.rs` |
| `config/` | 4 | 825 | Config loading + defaults |
| `identity/` | 1 | 499 | `FileId` / `FileIdentityMap` path↔id interning, `EdgeKind` |
| `graph/` | 2 | 459 | Dependency-graph + cycle primitives |
| `convention/` | 1 | 232 | Naming/convention inference |
| `extractor/` | 1 | 90 | `ParserCache` extraction entry |

| `fmm-cli/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `cli/` | 34 | 5,512 | clap `Commands` enum + per-command `commands/<cmd>.rs` handlers |
| `mcp/` | 31 | 4,297 | MCP server: the same queries exposed as agent tools |
| `src/` (root) | 9 | 1,503 | `main.rs`, `read_symbol.rs`, `glossary.rs`, `git.rs`, `cycle_report.rs`, `outline_freshness.rs`, fs/glob utils |
| `read_symbol/` | 1 | 569 | `member_error.rs` member-resolution diagnostics |

| `fmm-store/src` module | Files | LOC | Role |
|---|---:|---:|---|
| `src/` (root) | 7 | 2,342 | `lib.rs`, `sqlite_store.rs`, `writer.rs`, `connection.rs`, `error.rs`, `schema` |
| `reader/` | 6 | 732 | Loading manifest rows back out of SQLite |
| `memory_store/` | 4 | 716 | `InMemoryStore` (tests + non-persistent use) |

## Key components (high fan-in — the spine)

Ranked by direct dependents (`fmm ls --sort-by downstream --filter source`). These are the highest-blast-radius edit surfaces. Reverse-transitive closure via `fmm deps <file> --reverse --transitive --filter source`.

| Component (`path Symbol`) | ↓ direct | ↓ transitive | Role / why it is load-bearing |
|---|---:|---:|---|
| `parser/mod.rs` (re-export facade) | 83 | 135 | 12-LOC facade that `pub use`s the parser type vocabulary (`Parser`, `ParserRegistry`, `ExportEntry`, `Metadata`, `ParseResult`, `DeclarationKind`, `SymbolVisibility`). Nearly every core file imports parser types through it; renaming a re-export ripples crate-wide. |
| `manifest/mod.rs Manifest` | 81 | 87 | The in-memory index model: `files`, `export_index`, `export_locations`, `file_identity`, reverse-deps. The single source of truth every query reads. Changing its shape reaches store, search, resolver, and CLI. |
| `identity/mod.rs FileIdentityMap` | 29 | — | Path↔`FileId` interning + `EdgeKind {Runtime, TypeOnly}`. Underpins the dependency graph and the type-only-vs-runtime edge distinction (`--edge-mode`). |
| `fmm-store memory_store/manifest.rs build_manifest` | 25 | — | Rebuilds a `Manifest` from stored rows — the read path's core. |
| `parser/builtin/query_helpers.rs` | 20 | — | Shared tree-sitter query utilities reused by every language parser. |
| `store.rs FmmStore` | 18 | — | The persistence **port** (trait) with associated `type Error`. Also owns `GitMeta` + `GIT_*_META_KEY` (the index git-stamp data model). |
| `resolver/mod.rs ImportResolver` | 17 | — | Import-resolution trait + `CrossPackageResolver`; layers deno/go/rust/workspace resolution. |
| `config/mod.rs`, `cli/commands/mod.rs`, `search/mod.rs`, `types.rs` | 15 / 13 / 12 / 11 | — | Config surface, the clap `Commands` enum, query result types, shared core types. |

## Seams & boundaries

- **Persistence port/adapter (clean inversion).** `FmmStore` (`fmm-core/src/store.rs`) declares the contract — associated `type Error`, `load_manifest`, `load_fingerprints`, `update_file_fingerprint`, batch row writers, and `write_meta(Option<&GitMeta>)` for the git stamp. Adapters `SqliteStore` and `InMemoryStore` live in `fmm-store` and depend on core's trait; **core never depends on `fmm-store`.** Safest place to add a new backend.
- **Parser plug-in boundary.** `Parser` trait (`fmm-core/src/parser/types.rs`: `parse` / `parse_file` / `language_id` / `extensions`, `Send + Sync`). Each language is an impl under `parser/builtin/` (71 files); `ParserRegistry` (`parser/registry.rs`) maps language id → factory. Adding a language = new builtin impl + registry entry; no other surface changes.
- **Resolution boundary.** `ImportResolver` trait (`resolver/mod.rs`: `resolve(&self, importer, specifier) -> Option<PathBuf>`) with deno/go/rust/workspace layer impls behind `CrossPackageResolver`.
- **Two front-ends, one core.** `fmm-cli/src/cli/` (clap; `Cli` → `Commands` enum → per-command handlers) and `fmm-cli/src/mcp/` (MCP tools) are parallel surfaces over `fmm-core`. Entry: `main.rs main()` → `run()` → `run_command(Commands)`. New behavior should land in core, then be exposed by both front-ends.
- **Dependency cycles (now visible).** `fmm cycles` defaults to runtime edges and hides module-hierarchy facades, so its output is now the *real* coupling, not idiomatic `mod.rs`↔submodule re-exports. Two SCCs remain under `--filter source`:
  - `resolver/deno.rs ↔ resolver/workspace.rs` — **genuine.** The deno layer `use`s `../workspace` and the workspace layer `use`s `../deno`; mutual recursion across two resolution layers. Watch this if the resolver grows.
  - `format/search_formatters.rs ↔ cli/commands/dupes.rs` — **a matcher artifact, not real coupling.** `search_formatters.rs` imports `crate::dupes::DupeClustersResult` from core's `dupes.rs`; the basename collision with the CLI's `cli/commands/dupes.rs` makes the dependency matcher draw a phantom `core → cli` edge, which is impossible as a real import (`fmm-core` cannot depend on `fmm-cli`). The genuine edge is `cli/commands/dupes.rs → format/search_formatters.rs`. A dogfood finding: fmm's own new `dupes` name confuses its by-name matcher. (`fmm cycles --filter source --explain` shows the closing edges; `--include-mod-hierarchy` restores the facades.)

## Public API surface

- **`fmm-core`** (`lib.rs` public modules): `config, convention, dupes, error, extractor, format, graph, identity, manifest, parser, resolver, search, similarity, store, types` + `VERSION`. Headline types: `Manifest`, `FileEntry`, `Parser`, `ParserRegistry`, `FmmStore`, `GitMeta`, `ImportResolver`, `FileId`, `FmmError`. `dupes` is the newest public module (the repo-wide duplicate scorer).
- **`fmm-store`** (`lib.rs` re-exports): `SqliteStore`, `InMemoryStore`, `StoreError`, `MemoryStoreError`, `open_db`, `DB_FILENAME`; modules `connection / error / memory_store / sqlite_store / writer / reader`.
- **`fmm-cli`** (binary; the user-facing contract): subcommands `ls, outline, lookup, exports, read, deps, search, glossary, similar, cycles, dupes, generate, validate, watch, init, clean, status, mcp, completions, sidecar`, plus the matching **11 MCP tools** (`fmm_dupe_clusters` is the newest). The MCP tool set is generated from `crates/fmm-cli/tools.toml` via `build.rs` — that file is the single source of truth for tool names, CLI aliases, and params.

## Patterns & conventions

- **Thin re-export facade modules.** `mod.rs` declares submodules and `pub use`s a curated vocabulary. Cite: `parser/mod.rs` (12 LOC, re-exports the parser types), `fmm-core/src/lib.rs`.
- **Ports as traits, impls elsewhere.** `FmmStore`, `Parser`, `ImportResolver`. `FmmStore` uses an associated `type Error` so each adapter defines its own error type.
- **Layered error strategy.** Typed errors at library boundaries via `thiserror` — `FmmError` (`fmm-core/src/error.rs`) and `StoreError` (`fmm-store/src/error.rs`) — and `anyhow` at the binary (`fmm-cli/src/main.rs` `run` / `run_command`).
- **Newtype identity / interning.** `FileId`, `RelativePath`, `FileIdentityMap`. Cite: `identity/mod.rs`.
- **Registry pattern.** `ParserRegistry` (`parser/registry.rs`).
- **Scorer reuse over re-implementation.** `dupes.rs` runs the existing `similarity.rs` structural scorer in batch mode (block by kind / rare name tokens / signature shape, then union-find clusters) rather than a second similarity engine. Cite: `fmm-core/src/dupes.rs`, `fmm-core/src/similarity.rs`.
- **CLI handler-per-command.** Each subcommand's args + handler live in `cli/commands/<cmd>.rs`; the MCP mirror lives in `mcp/tools/<cmd>.rs`. New commands (e.g. `dupes`) add both.
- **Test layout.** Inline `#[cfg(test)] mod tests` inside source files plus a top-level `fixtures/` tree. fmm's own `--filter source|tests` is path-based, so inline test blocks do not mark a source file as a test file. Run tests with `just test` (nextest), **not** `cargo test`.

## Health flags (candidates, not verdicts)

- **Heaviest source files** (none exceed the repo's 700-line ceiling; largest 645): `similarity.rs` (645), `parser/builtin/python/mod.rs` (629), `dupes.rs` (617), `cli/read_symbol.rs` (608), `format/yaml_formatters.rs` (598), `manifest/glossary_builder.rs` (580), `read_symbol/member_error.rs` (569), `cli/watch.rs` (565), `fmm-store/writer.rs` (563), `resolver/deno.rs` (547), `parser/builtin/go.rs` (541), `fmm-store/sqlite_store.rs` (531), `parser/builtin/kotlin.rs` (501), `resolver/workspace.rs` (499). Worth a glance as they approach the limit.
- **Cycles:** 2 real runtime SCCs (see Seams). One genuine (`resolver/deno.rs ↔ workspace.rs`), one a by-name matcher artifact (`search_formatters.rs ↔ cli/commands/dupes.rs`).
- **Duplication candidates** (`fmm dupes`, default threshold — 8 clusters; recall, not verdicts):
  - `signature_end_byte` is **triplicated** across `parser/builtin/{python,rust,typescript}/symbol_metadata.rs` (score 1.00). The genuinely consolidatable one — a shared helper or trait default would remove three copies.
  - `VERSION` const matches across `fmm-cli/src/lib.rs` and `fmm-core/src/lib.rs` (score 1.00) but is **intentional**: each reads a different `env!` source. A candidate, not a defect.
  - The `PrivateMemberExtractor` family (`extract_top_level_functions`, `extract_private_members`, `extensions`) across `manifest/private_members/{mod,python,typescript}.rs` (0.90–0.94) and `language_id` across `c.rs`/`csharp.rs` (0.90) are per-language trait impls — expected structural similarity, likely fine as-is.

---
*Generated from fmm structural primitives at `29990d0`. To refresh after a commit: `git diff --name-only 29990d0..HEAD`, `fmm generate`, then re-run `fmm outline`/`fmm deps` on changed files and patch only the affected sections + re-stamp the header.*
