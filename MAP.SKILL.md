---
name: map
description: Generate (or refresh) a MAP.md that orients an LLM agent to a codebase fast — key components, seams and boundaries, coding patterns, public surface — stamped with the git SHA it reflects. Use when asked to map a repo, produce a MAP.md / repo map / onboarding map, or keep an existing MAP.md current after commits. Built on fmm structural primitives.
---

# Generate a codebase MAP.md

A MAP.md is written **for an LLM agent**, not a human onboarding doc. Its job: let an agent that has never seen this repo know, in one read, where the load-bearing code is, what the boundaries are, what not to break, and where to start. fmm gives you the structural facts; **you write the narrative**. Never paste raw fmm output into the map.

## The division of labor (do not violate)

- **fmm** answers structural questions deterministically: module topology, fan-in/fan-out, cycles, exports, symbol sizes, who-imports-what. Use it instead of grepping or reading whole files.
- **You** decide what matters, infer each component's *role*, name the seams, and write prose. Ranking, judgment, and explanation are yours.
- A map that just dumps `fmm` tables is a failure. A map that explains *why parser/mod.rs is the spine of the system and what depends on it* is the goal.

## Preconditions

```bash
fmm validate          # exit 0 = index current; exit 1 = stale/missing
fmm generate          # refresh if stale (mtime-incremental, cheap)
```

If `fmm` is not set up (`./.fmm.db` absent), stop and say so — this skill needs the index.

## Step 1 — Stamp the map

Read the commit the map reflects so a future reader knows if it is stale. Stamp from **git directly** — it is always current and does not depend on the installed `fmm` version:

```bash
git rev-parse --short HEAD        # sha
git rev-parse --abbrev-ref HEAD   # branch (HEAD if detached)
git status --porcelain            # non-empty => dirty
```

(If the installed `fmm` is current, `fmm status` also surfaces the SHA the index was built against — use it as a cross-check that the index matches HEAD, but git is the source of truth for the stamp.) If the tree is not a git repo, omit sha/branch — git is optional.

Every MAP.md begins with a stamp header (HTML comment so it renders cleanly):

```markdown
<!-- fmm:map sha=<short-sha> branch=<branch> dirty=<true|false> generated=<iso-date> files=<n> loc=<n> -->
```

If `Dirty: true`, note in the header that the map may not byte-correspond to the commit. If the tree is not a git repo, omit sha/branch (git is optional — `fmm status` simply won't show the section).

## Step 2 — Gather structural facts

Run these; keep the outputs as your evidence, do not transcribe them verbatim into the map.

| Question | Command |
|---|---|
| Module inventory + size | `fmm ls --group-by subdir` |
| Heaviest dirs/files (god-files) | `fmm ls --sort-by loc --filter source` |
| **Load-bearing components** (high fan-in) | `fmm ls --sort-by downstream --filter source --limit 20` |
| What a hub depends on / its blast radius | `fmm deps <file> --depth 2 --filter source` |
| **Seams / coupling clusters** | `fmm cycles --filter source` (and `--edge-mode all` for type-only edges) |
| Public API surface of a package | `fmm exports --dir <dir>` / `fmm exports '^Pattern'` |
| Impact / importers of a key symbol | `fmm glossary <Symbol> --precision call-site` |
| Shape of a component (signatures, visibility, kinds) | `fmm outline <file> --include-private` |
| Exact definition / source when needed | `fmm lookup <Symbol>` / `fmm read <Symbol>` |

Heuristic for "key components": start from `fmm ls --sort-by downstream`. The top files are the ones most of the codebase imports — they are the spine. For each, use `fmm deps` and `fmm outline` to understand its role, then write one or two sentences naming that role.

`--json` is available on these commands if you prefer to parse rather than read.

## Step 3 — Assemble MAP.md

Write these sections. Prose explains role and intent; tables carry the numbers.

1. **Overview** — 2-4 sentences: what this codebase is, its top-level shape (e.g. "Rust workspace: `fmm-core` library + `fmm-cli` binary + `fmm-store` persistence"), and the one place a new agent should start reading.
2. **Topology** — directory→(files, LOC) table from `ls --group-by subdir`, with a sentence per major area saying what lives there.
3. **Key components** — the high-fan-in hubs. For each: file path, downstream count, and a prose line on its responsibility and why touching it is high-blast-radius. This is the most valuable section for an agent.
4. **Seams & boundaries** — the contracts between components: cross-module dependency edges, any dependency cycles (from `fmm cycles` — name them, they are coupling debt), and the key interfaces/traits that define the boundaries. Tell the agent where the safe edit surfaces are vs the load-bearing ones.
5. **Public API surface** — top exports per package (`fmm exports`), so an agent knows the stable contract vs internals.
6. **Patterns & conventions** — coding patterns you *infer* from outlines (error strategy, newtype/builder, trait organization, test layout). Cite file+symbol examples. Only claim a pattern you can point to.
7. **Health flags (candidates, not verdicts)** — god-files over the repo's size limit, dependency cycles, obvious duplication candidates. Frame as "worth a look," not judgments.

### Reference code by **path + symbol, never line numbers**

Line numbers rot on the next commit; `crates/fmm-core/src/parser/mod.rs ParserRegistry` survives and is greppable. Use it everywhere.

## Step 4 — Keep it fresh after a commit (no full regen)

When a new commit lands on main, do not regenerate the whole map. Diff and patch only what changed:

```bash
git diff --name-only <map-header-sha>..HEAD     # files that changed since the map's stamp
fmm generate                                     # refresh the index
```

For each changed file, re-run `fmm outline <file>` / `fmm deps <file>` and update only the MAP sections that mention it (or whose hub/topology numbers moved). Then re-stamp the header with the new SHA. If `git diff` shows only leaf changes, the topology/components sections usually need no edit. Reserve a full re-map for large structural shifts (new crate, hub moved, cycles appeared).

## Duplication candidates (recall, not verdicts)

When the map's health section should flag duplication, fmm surfaces *candidates*; you judge which are real:

```bash
fmm similar <Symbol>                              # existing symbols structurally like this one
fmm similar <name> --signature "(Path) -> Config" --kind fn   # pre-write probe
```

`fmm similar` today is **probe-based** (one symbol at a time). There is no repo-wide duplicate scan yet. If during a real run you find yourself wanting "show me every near-duplicate cluster in the repo" or "every function over 150 lines," **stop and record it** — that is a missing fmm primitive (planned: `fmm dupes`, `fmm symbols`/`body_loc`), and the friction is the signal for what to build next. Do not brute-force it by reading dozens of files.

## Output

Write to `MAP.md` at the repo root (or the path requested). Print the stamp header, then the sections. The map should be committable, deterministic given the same SHA, and readable top-to-bottom by an agent in under a minute.

## Anti-patterns

| Don't | Do |
|---|---|
| Paste raw `fmm` tables as the map | Synthesize; explain role and intent |
| Cite `file:line` | Cite `file Symbol` |
| Read whole files to understand structure | `fmm outline` / `fmm deps` first |
| Brute-force duplicate detection by reading files | Use `fmm similar`; record the gap if it is insufficient |
| Editorialize beyond evidence | Every claim points to an fmm fact |
| Regenerate the whole map every commit | `git diff` + patch the affected sections |
