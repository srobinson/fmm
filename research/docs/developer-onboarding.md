# Developer Onboarding Research: World-Class CLI First-Run Experiences

> Research compiled January 2026. Focus: what makes developers install, try, adopt, and depend on a CLI tool — and concrete recommendations for fmm.

---

## Table of Contents

1. [First-Run Experience (FTRE)](#1-first-run-experience-ftre)
2. [Time to First Value (TTFV)](#2-time-to-first-value-ttfv)
3. [Progressive Complexity](#3-progressive-complexity)
4. [Error Messages as Documentation](#4-error-messages-as-documentation)
5. [Interactive Tutorials and Playgrounds](#5-interactive-tutorials-and-playgrounds)
6. [README as Onboarding](#6-readme-as-onboarding)
7. [Measuring Onboarding Success](#7-measuring-onboarding-success)
8. [fmm Gap Analysis and Recommendations](#8-fmm-gap-analysis-and-recommendations)

---

## 1. First-Run Experience (FTRE)

The first 60 seconds after installation determine whether a developer becomes a user or uninstalls. The best CLI tools treat first-run as a product surface, not an afterthought.

### 1.1 Patterns from Best-in-Class Tools

#### `cargo init` — Scaffolding with Sensible Defaults

- Creates `Cargo.toml`, `src/main.rs` with a working "Hello, world!" in one command
- Zero prompts, zero configuration, immediately compilable
- The key insight: **the output of init is a working program**, not a config file
- Teaches the project structure implicitly (src/, Cargo.toml convention)

**What fmm can learn**: `fmm init` currently creates config files (.fmmrc.json, skill, MCP). It should also *generate sidecars* so the user sees immediate output — the "Hello, world!" equivalent for fmm is seeing your first `.fmm` sidecar file.

#### `gh auth login` — Interactive Guided Setup

- Detects environment (browser available? CI? SSH keys present?)
- Offers choices with sensible defaults highlighted
- Falls back gracefully (no browser -> token paste, no credential store -> plain text)
- After auth completes, tells you exactly what to do next

**What fmm can learn**: fmm has no interactive prompts. For a tool that needs zero auth, this is fine. But the *post-init guidance* pattern matters: tell the user exactly what to do next, not just "Setup complete!"

#### `npx create-next-app` — Interactive Project Creation

- Running with no arguments launches an interactive wizard
- Running with flags (`--typescript --tailwind`) is fully non-interactive (CI-friendly)
- Every prompt has a visible default so Enter-to-continue works
- The final output is a running application, not just files

**What fmm can learn**: Dual-mode design (interactive for humans, flags for CI) is table stakes. fmm's `init` supports `--skill`, `--mcp`, `--all` which is good. Missing: the interactive path for someone who just types `fmm init` with no flags.

#### `deno init` — Instant Project Scaffold

- Creates `main.ts`, `main_test.ts`, and `deno.json` — three files, done
- The main file is a working program; the test file tests it
- No prompts, no choices, immediate value
- Under 1 second wall time

**What fmm can learn**: Speed is a feature. fmm's init is already fast, but should feel instant. Consider: what if `fmm init` also ran `fmm generate` automatically and showed the user their first sidecar?

#### `wrangler init` — Cloudflare's Interactive Setup

- Asks 4-5 focused questions (Git? Package.json? TypeScript? Handler type?)
- Each question has a default; you can Enter through the entire flow
- Creates deployable code, not just config
- Now being replaced by `create-cloudflare-cli` (C3) which is even more streamlined
- OAuth login flow uses PKCE with browser redirect — smooth when it works

**What fmm can learn**: Cloudflare learned that even interactive setup has diminishing returns. They moved to a simpler, faster flow (C3). fmm should stay on the "fast defaults" side.

#### `claude` (Claude Code) — First-Run Onboarding

- First run detects environment, sets up auth via browser
- Progressive disclosure: starts simple, reveals MCP/skills as user matures
- Uses CLAUDE.md as layered entry point — each project can customize what Claude sees
- Skill files act as "progressive disclosure layer 2" — loaded on demand, not upfront

**What fmm can learn**: Claude Code's progressive disclosure of skills is directly relevant. fmm's skill file (`fmm-navigate.md`) is the mechanism Claude uses to learn about fmm. The init flow should explain this relationship clearly.

### 1.2 fmm's Current Init Flow — Audit

Current `fmm init` does three things:
1. Creates `.fmmrc.json` with default config
2. Installs `.claude/skills/fmm-navigate.md` (the Claude Code skill)
3. Creates/updates `.mcp.json` with fmm server entry

**What's good:**
- Idempotent (skips existing files)
- Merges into existing `.mcp.json` instead of overwriting
- Shows clear status messages with checkmarks
- Ends with a clear next step: "Run `fmm generate` to create sidecar files."

**What's missing:**
- No auto-generation of sidecars (the "Hello World" moment is deferred)
- No summary of what fmm *is* for first-time users
- No detection of project characteristics (language, file count, size estimate)
- No "here's what you'll get" preview before generating
- No post-init validation ("fmm is ready — 247 source files detected, run `fmm generate` to create sidecars")

### 1.3 Recommended FTRE for fmm

```
$ fmm init

  Frontmatter Matters — metadata sidecars for LLM code navigation

  Scanning project...
    247 source files detected (TypeScript, Python, Rust)
    Estimated sidecar size: ~42 KB total

  Setup:
    [checkmark] .fmmrc.json (default configuration)
    [checkmark] .claude/skills/fmm-navigate.md (Claude Code skill)
    [checkmark] .mcp.json (MCP server entry)

  Generating sidecars...
    [checkmark] 247 sidecars written in 0.16s

  Done! Your AI assistant now navigates via metadata.

  Try it:
    fmm search --export createUser    Find where a symbol is defined
    fmm status                        See project overview
    fmm validate                      Check sidecars are current (CI-ready)
```

Key changes from current behavior:
- **Auto-generate sidecars** as part of init (the value is immediate)
- **Show project stats** so the user knows fmm understood their codebase
- **Suggest next commands** ranked by likelihood of use
- **One-line value prop** at the top for first-time users

---

## 2. Time to First Value (TTFV)

### 2.1 Definition for CLI Tools

TTFV = time between `cargo install fmm` (or `brew install fmm`) completing and the user experiencing meaningful value.

For fmm, "first value" means one of:
- Seeing a sidecar file and understanding its purpose
- Running a search query and getting an instant answer
- Having an AI assistant navigate their codebase faster

### 2.2 TTFV Benchmarks

| Tool | Install-to-Value | What "Value" Means |
|------|-----------------|-------------------|
| ripgrep | ~2 seconds | First search result appears |
| bat | ~2 seconds | First file displayed with syntax highlighting |
| cargo init | ~3 seconds | Working project scaffold |
| create-next-app | ~30-60 seconds | Running dev server |
| gh auth login | ~30-60 seconds | Authenticated, can run `gh pr list` |

**fmm today**: ~15-30 seconds (install -> init -> generate -> look at sidecar). This is good. It should be ~5-10 seconds with auto-generate in init.

### 2.3 Strategies to Minimize TTFV

**Strategy 1: Collapse init + generate into one step**

Current:
```bash
cargo install fmm    # 30s+ (compile time)
fmm init             # 1s
fmm generate         # 0.5s
# now look at a sidecar file... which one?
```

Proposed:
```bash
cargo install fmm    # 30s+ (compile time, unavoidable)
fmm init             # 2s (includes generate + shows sample sidecar)
```

**Strategy 2: Demo repository**

Create an `examples/demo-project/` in the fmm repo with:
- A small multi-language project (5-10 files)
- Pre-generated sidecars
- A README showing the navigation workflow
- Users can `cd examples/demo-project && fmm status` to see it working without touching their own code

**Strategy 3: Show, don't tell**

After generating sidecars, print one example:
```
  Example sidecar (src/auth/session.ts.fmm):
    exports: [createSession, validateSession, destroySession]
    imports: [jwt, redis-client]
    dependencies: [./types, ./config]
    loc: 234
```

This removes the "now what?" gap between generation and understanding.

**Strategy 4: Instant search demo**

After init, suggest a search with a real symbol from the user's codebase:
```
  Try: fmm search --export "validateSession"
```

Pick the most "interesting" export (one with dependencies, not a trivial utility).

### 2.4 The "Hello World" for fmm

Every tool type has its "Hello World" moment:

| Tool Type | Hello World |
|-----------|------------|
| Language | `print("Hello, world!")` |
| Web framework | Running dev server + seeing page |
| Formatter | Before/after diff |
| Linter | First warning with fix suggestion |
| **fmm** | **Seeing your first sidecar, then finding a symbol via search** |

fmm's Hello World is a two-step experience:
1. "Oh, it generated metadata about my code" (recognition)
2. "Oh, I can find things instantly" (utility)

Both should happen within the first 10 seconds.

---

## 3. Progressive Complexity

### 3.1 Theory: Progressive Disclosure

Progressive disclosure (Jakob Nielsen, 1995) reduces cognitive load by revealing complexity only when the user is ready. Research shows:
- Human working memory holds 5-9 items
- More than 2 disclosure levels degrades usability
- Beginners need simplicity; experts resent over-explanation

For CLI tools, progressive disclosure means:
- **Level 0**: Core command works with zero flags
- **Level 1**: Useful flags appear in `--help`
- **Level 2**: Config files, plugins, integrations

### 3.2 How Best Tools Layer Complexity

**Git** (the canonical example of progressive CLI complexity):
- Day 1: `git init`, `git add`, `git commit`
- Day 30: branches, merges, rebases
- Day 365: reflog, bisect, worktrees
- Most users never need 80% of git

**Docker**:
- Day 1: `docker run hello-world`
- Day 7: Dockerfile, build, push
- Day 30: docker-compose, networks, volumes
- Day 90: Swarm, multi-stage builds

**Rust/Cargo**:
- Day 1: `cargo new`, `cargo run`
- Day 7: Dependencies in Cargo.toml, `cargo test`
- Day 30: Features, workspaces, custom profiles
- Day 90: Proc macros, build scripts, cross-compilation

### 3.3 fmm's Progressive Complexity Map

**Day 1: See value**
```bash
fmm init                          # Generate everything
fmm search --export createUser    # Find a symbol
```
User thinks: "This is useful, it found the file instantly."

**Day 2: Navigate codebase**
```bash
fmm search --imports crypto       # Find all files importing X
fmm search --loc ">500"           # Find large files
fmm search --depends-on ./types   # Find dependency consumers
```
User thinks: "I can answer structural questions without reading code."

**Day 3: AI integration**
```
# In Claude Code, the skill file auto-teaches the AI about fmm
# AI starts using fmm_lookup_export instead of grep
# User notices fewer file reads, faster responses
```
User thinks: "My AI assistant is significantly more efficient."

**Day 4: CI/CD and team workflow**
```yaml
# .github/workflows/ci.yml
- name: Validate sidecars
  run: fmm validate src/
```
```yaml
# .pre-commit-config.yaml
- id: fmm-update
  entry: fmm update
```
User thinks: "Sidecars stay fresh automatically, the whole team benefits."

**Day 5+: Power usage**
```bash
fmm compare https://github.com/org/repo   # Benchmark fmm's impact
fmm gh issue https://github.com/.../42     # AI-powered issue fixing
fmm mcp                                    # Custom MCP server
```

### 3.4 What fmm Should Do at Each Layer

| Layer | Commands Visible | Docs Needed | Error Detail |
|-------|-----------------|-------------|--------------|
| 1 (First use) | init, generate, search | Quick Start only | "Run `fmm init` first" |
| 2 (Daily use) | update, validate, status, clean | CLI reference | Sidecar-specific diagnostics |
| 3 (Integration) | mcp, serve, gh | Integration guides | Config troubleshooting |
| 4 (Power user) | compare, advanced search | Architecture docs | Full debug output |

### 3.5 Recommendations for fmm

1. **`fmm --help` should show only Layer 1-2 commands by default.** Use `fmm --help-all` or grouped help for Layer 3-4. Clap supports subcommand groups.

2. **The README should present commands in complexity order**, not alphabetical. Current README does this well.

3. **Error messages should reference the user's current layer.** If someone hasn't run `fmm init` yet, every error should say "Run `fmm init` first" — not show a Rust backtrace.

4. **Config should be optional.** fmm already does this (defaults work without .fmmrc.json). Keep it that way.

---

## 4. Error Messages as Documentation

### 4.1 Elm's Approach: Errors as Teaching Moments

Elm's compiler treats every error as an opportunity to educate. Key principles:

- **Plain English, not jargon**: "I cannot find a `String.toInt` function" not "unresolved reference"
- **Show the code in context**: Highlight the exact line and column
- **Suggest a fix**: "Maybe you want String.fromInt?" with a confidence level
- **Link to docs**: "Read more at [url]" for complex concepts
- **Tone matters**: Friendly, not condescending. "I ran into something unexpected" not "ERROR"

Criticism: Elm's verbosity treats everyone as a novice. For experts, this becomes noise. The 80/20 framework addresses this: 80% of errors are obvious from location alone; 20% need detailed explanation.

### 4.2 Rust's Approach: Structured Diagnostics

Rust's compiler uses a formal diagnostic hierarchy:

- **error**: What went wrong (the "what")
- **note**: Additional context, facts, links (the "why")
- **help**: Actionable fix suggestion (the "how")
- **suggestion applicability levels**: MachineApplicable (safe to auto-fix) through MaybeIncorrect (needs human judgment)

Key design rules from the Rust compiler development guide:
- The error line should NOT suggest a fix — only the `help` sub-diagnostic should
- Avoid "did you mean" — prefer "there is a struct with a similar name: Foo"
- Never say "the following" or "as shown" — use the span to point at code
- Color-code labels: red for errors, blue for context, green for suggestions

### 4.3 Current fmm Error Patterns

Auditing fmm's current error messages:

```rust
// Good: clear action message
eprintln!("{} {}: {}", "Error".red(), file.display(), e);

// Good: validation with actionable context
println!("  {} {}: {}", "✗".red(), rel.display(), "sidecar out of date");

// Good: idempotent skip message
println!("{} .fmmrc.json already exists (skipping)", "!".yellow());

// Missing: no suggestion for what to do next on error
anyhow::bail!("Sidecar validation failed");
// Better: "Sidecar validation failed. Run `fmm update` to fix."

// Missing: no detection of common mistakes
// e.g., running `fmm generate` in wrong directory, no source files found
```

### 4.4 Error Message Design System for fmm

**Principle 1: Every error should include a recovery action**

```
Bad:  Error: No source files found
Good: No source files found in ./src
      fmm supports: .ts .tsx .js .jsx .py .rs .go .java .cpp .cs .rb
      Check your path or add languages to .fmmrc.json
```

**Principle 2: Detect the most common mistakes**

| Mistake | Current Behavior | Recommended Behavior |
|---------|-----------------|---------------------|
| No sidecars exist | "No matches found" | "No sidecars found. Run `fmm generate` first." |
| Wrong directory | Silently processes 0 files | "Found 0 source files in /wrong/path. Did you mean to run from your project root?" |
| Stale sidecars | validate fails with count | "47 sidecars are stale. Run `fmm update` to refresh, or `fmm update -n` to preview changes." |
| Missing config | Uses defaults silently | (Keep current behavior: defaults are correct. Show in `fmm status` only.) |
| Binary in path | Skips silently | (Keep current behavior: correct.) |

**Principle 3: Use structured output for machine consumption**

```
# Human output (default)
[X] 3 files need updating:
  [X] src/auth/session.ts: sidecar out of date
  [X] src/api/routes.ts: missing sidecar
  [X] src/lib/utils.ts: sidecar out of date

Run `fmm update` to fix all issues.

# Machine output (--json flag)
{"status":"fail","stale":2,"missing":1,"files":["src/auth/session.ts","src/api/routes.ts","src/lib/utils.ts"]}
```

**Principle 4: Progressive error detail**

```
# Default: concise
Error: Parse failed for src/weird.ts (unsupported syntax)

# With FMM_LOG=debug or --verbose:
Error: Parse failed for src/weird.ts
  tree-sitter returned 3 ERROR nodes at lines 45, 89, 112
  This usually means the file contains syntax not yet supported by the TypeScript grammar
  File size: 1,247 lines | Language: typescript
  Report at: https://github.com/srobinson/fmm/issues
```

### 4.5 Specific Error Messages to Add

1. **No project detected**: "This directory doesn't appear to be a code project (no source files matching supported languages). Run `fmm status` to see supported languages, or specify a path: `fmm generate ./src`"

2. **Init already complete**: Change "already exists (skipping)" to "Already set up. Your project has 247 sidecars covering 12 languages. Run `fmm status` for details."

3. **Search with no sidecars**: "No sidecars found. fmm searches metadata files, not source code. Run `fmm generate` to create them (takes <1 second for most projects)."

4. **MCP server start failure**: "Failed to start MCP server: [reason]. Check that no other fmm instance is running. The MCP server communicates via stdio — it should be started by your AI tool, not manually."

---

## 5. Interactive Tutorials and Playgrounds

### 5.1 The Landscape

| Approach | Examples | Strengths | Weaknesses |
|----------|----------|-----------|------------|
| Web REPL | Rust Playground, Go Tour, Svelte REPL | Zero install, instant feedback | Doesn't reflect real workflow |
| Guided tutorial | Katacoda (defunct), Instruqt | Real environment, step-by-step | Heavy infrastructure, costs money |
| Recorded demos | asciinema, VHS (charm.sh) | Lightweight, embeddable, copy-paste | Non-interactive, can go stale |
| Example repos | Many tools | Real-world context | Requires clone + setup |
| In-CLI tutorial | `rustlings`, `go tour` | Meets user where they are | Hard to maintain |

### 5.2 What Works for CLI Tools Specifically

For a CLI tool like fmm, the most effective approaches are:

**Asciinema recordings** (highest ROI):
- Record `fmm init && fmm generate && fmm search --export createUser` on a real project
- Embed in README and docs site
- Viewers can copy commands directly from the recording
- Lightweight: a 2-minute recording is ~50KB vs 10MB+ for a GIF
- The current asciinema (3.x) is written in Rust — thematic alignment with fmm
- Can be automated with `asciinema-automation` for reproducible recordings

**Example project in-repo** (medium effort, high value):
- `examples/demo-project/` with pre-generated sidecars
- Include a `WALKTHROUGH.md` showing the navigation workflow
- Users can experiment without risking their own codebase
- Also serves as a test fixture

**SVG/GIF for README** (essential for GitHub discovery):
- Use `svg-term-cli` or `agg` to convert asciinema recordings to animated SVG/GIF
- SVGs are resolution-independent, smaller than GIFs
- Place above the fold in README

### 5.3 What Doesn't Work for CLI Tools

- **Web REPLs**: fmm operates on local files and integrates with local tools (Claude Code, MCP). A web playground would demonstrate a fraction of the value.
- **Katacoda-style labs**: Overkill for a tool with a 5-second setup. The infrastructure cost doesn't justify the marginal improvement over asciinema.
- **In-CLI tutorials**: fmm's surface area is small enough that `--help` + good error messages + example commands cover the tutorial need.

### 5.4 Recommendations for fmm

1. **Create 3 asciinema recordings**:
   - "Getting started" (init + generate + first search) — 30 seconds
   - "Navigating a codebase" (search by export, import, dependency, LOC) — 60 seconds
   - "AI integration" (Claude Code using fmm to navigate) — 90 seconds

2. **Convert the "Getting started" recording to animated SVG** for the README

3. **Add `examples/demo-project/`** with:
   - 8-10 files across TypeScript, Python, Rust
   - Pre-generated `.fmm` sidecars
   - A `WALKTHROUGH.md` showing 5 common queries and their results

4. **Do NOT build** a web playground, in-CLI tutorial, or interactive sandbox. The effort-to-value ratio is wrong for fmm's simplicity.

---

## 6. README as Onboarding

### 6.1 What Makes a README Convert Visitors to Users

The README is the single most important onboarding surface for a CLI tool on GitHub. Research from the `awesome-readme` curation and analysis of top Rust CLI tools reveals consistent patterns.

**The 7-Second Test**: A visitor decides whether to keep reading within 7 seconds. In that time, they need to understand:
1. What the tool does (one sentence)
2. Why they should care (proof of value)
3. How to install it (one command)

### 6.2 README Structure of Top CLI Tools

#### ripgrep (80K+ GitHub stars)

Structure:
1. One-line description + badges (CI, crates.io, packaging)
2. Quick links to docs, FAQ, regex syntax
3. "Why should I use ripgrep?" (value proposition with benchmarks)
4. "Why shouldn't I use ripgrep?" (honest limitations — builds trust)
5. Screenshot of output
6. Six benchmark tables (evidence)
7. Installation for every platform
8. Building from source

What works:
- **Honesty**: "Why shouldn't I use ripgrep?" is rare and builds enormous trust
- **Benchmarks front and center**: Developers respond to evidence, not claims
- **Platform-specific install**: Every user finds their exact command
- **Screenshot above the fold**: You see what it looks like before reading further

#### starship (50K+ GitHub stars)

Structure:
1. Centered logo + badges
2. One-line value prop: "The minimal, blazing-fast, and infinitely customizable prompt for any shell!"
3. Six key benefits as bullet points
4. Animated GIF showing the prompt in action
5. One-line install command
6. Shell-specific config (collapsible sections for 13 shells)
7. Contributing, sponsors, license

What works:
- **Animated GIF is the hero element**: You see the product working before reading anything
- **Six benefits are scannable**: Speed, customization, universality, intelligence, features, ease
- **Collapsible sections**: Keep the README scannable; details for those who need them
- **13 language translations**: Signals global adoption

#### bat, fd, zoxide (10-50K stars each)

Common patterns:
- GIF or screenshot within first 3 scroll lengths
- Comparison to the tool they replace (bat vs cat, fd vs find, zoxide vs cd)
- One-liner install
- Feature list as short bullets, not paragraphs

### 6.3 fmm's Current README — Audit

**Strengths:**
- Strong one-line value prop: "88-97% token reduction for LLM code navigation"
- Evidence table right at the top (token reduction data)
- Clear "The Problem" / "The Solution" framing
- Architecture diagram
- Economics section with dollar savings
- Comprehensive CLI reference

**Weaknesses:**
- No animated GIF or screenshot (the tool is invisible to scanners)
- Quick Start requires TWO commands after install (`init` + `generate`) — should be ONE
- No comparison to the "without fmm" workflow (the problem section describes it but doesn't show it)
- The badge row is sparse (only CI)
- No one-line install command (`cargo install --path .` is build-from-source, not a published crate)
- "The Problem" section is text-heavy; a before/after visual would be more compelling
- Roadmap includes checked items — consider moving completed items out

### 6.4 Recommended README Changes for fmm

**Priority 1: Add a hero visual**
- Record an asciinema demo of `fmm init` + `fmm search`
- Convert to animated SVG
- Place immediately after the value prop line

**Priority 2: One-command install + setup**
```bash
cargo install fmm && fmm init
```
(Requires publishing to crates.io and making `init` include `generate`)

**Priority 3: Before/After visual**
```
WITHOUT fmm:                          WITH fmm:
  "Where is createUser defined?"        "Where is createUser defined?"
  1. grep -r "createUser" src/          1. fmm search --export createUser
  2. Read 12 files                         -> src/api/users.ts
  3. Find it in users.ts
  4. Read users.ts (500 lines)
  5. ~50,000 tokens consumed              ~200 tokens consumed
```

**Priority 4: Add badges**
- CI (already have)
- Version (crates.io)
- License (MIT)
- Language count ("9 languages")
- Lines of code or test count (confidence signal)

**Priority 5: Restructure for scanning**
Move the architecture diagram and evidence tables below the fold. The first screen should be:
1. Value prop (one line)
2. Badges
3. Hero GIF/SVG
4. Install command
5. What you get (3-bullet summary)

---

## 7. Measuring Onboarding Success

### 7.1 The CLI Tool Activation Funnel

For a CLI tool like fmm, the funnel has distinct stages:

```
Discovery       ──>  100% (found the repo/tool)
Install         ──>  ~30-50% (bothered to install)
First Run       ──>  ~60-80% of installers (ran a command)
First Value     ──>  ~40-60% of first-runners (saw useful output)
Habitual Use    ──>  ~10-20% of first-value users (integrated into workflow)
Team Adoption   ──>  ~5-10% of habitual users (spread to team/CI)
```

Each transition has specific failure modes:

| Transition | Common Failure | How to Detect | How to Fix |
|------------|---------------|---------------|-----------|
| Discovery -> Install | README doesn't convince | GitHub traffic analytics | Better README (Section 6) |
| Install -> First Run | Build fails, confusing first step | Issue reports, no telemetry | Pre-built binaries, one-command start |
| First Run -> First Value | "Now what?" gap | Time between init and generate | Auto-generate in init (Section 1) |
| First Value -> Habitual | Doesn't integrate with workflow | Usage frequency in CI logs | MCP/skill auto-setup, pre-commit hooks |
| Habitual -> Team | Hard to mandate, unclear team value | PR adoption patterns | Team setup guide, CI validation template |

### 7.2 Activation Metrics for fmm

Since fmm is an open-source CLI without telemetry (and should stay that way), activation must be measured through proxies:

**Proxy metrics available now:**
- GitHub stars and forks (discovery -> interest)
- Issues and PRs from non-maintainers (adoption)
- `cargo install` download counts on crates.io (install)
- References in other repos' `.mcp.json` or CLAUDE.md files (integration)

**Proxy metrics from CI:**
- `fmm validate` in GitHub Actions (team adoption)
- `.fmm` files committed to repos (habitual use)
- Pre-commit hook references (workflow integration)

**Qualitative signals:**
- "How did you hear about fmm?" in issues/discussions
- Blog posts or tweets mentioning fmm
- Conference talks or tutorial references

### 7.3 Defining fmm's "Aha Moment"

The "aha moment" is when a user realizes the tool's core value. For different tool types:

| Tool | Aha Moment |
|------|-----------|
| ripgrep | "It's as accurate as grep but 10x faster" |
| Docker | "It runs the same everywhere" |
| GitHub Copilot | "It wrote the function I was thinking about" |
| **fmm** | **"My AI found the file in one query instead of 30 grep cycles"** |

fmm's aha moment is indirect — the user sees it through their AI assistant's behavior. This is both a strength (the value compounds silently) and a weakness (the user might not attribute the improvement to fmm).

**Recommendation**: After `fmm init`, suggest a specific test:
```
Test fmm's impact:
  1. Ask Claude: "What file exports createUser?"
  2. Watch tool calls — it should use fmm_lookup_export (1 call)
     instead of grep + read (10-30 calls)
```

### 7.4 Key Drop-Off Points and Mitigations

**Drop-off 1: Install friction**
- Current: `cargo install --path .` (requires clone + Rust toolchain)
- Fix: Publish to crates.io (`cargo install fmm`), provide Homebrew formula, pre-built binaries via GitHub Releases

**Drop-off 2: "Now what?" after init**
- Current: Init creates config files, says "Run `fmm generate`"
- Fix: Auto-generate sidecars in init, show a sample sidecar, suggest a search command with a real symbol from the project

**Drop-off 3: Invisible AI improvement**
- Current: User must notice that Claude is making fewer tool calls
- Fix: Add a `fmm compare` summary mode that shows before/after token usage for a specific task. Consider: `fmm compare --quick` that runs a single navigation task with and without sidecars

**Drop-off 4: Sidecars go stale**
- Current: User must remember to run `fmm update` or set up CI
- Fix: Provide a copy-paste GitHub Actions snippet in `fmm init` output. Provide a pre-commit hook config. Consider: `fmm watch` mode for auto-update on save.

---

## 8. fmm Gap Analysis and Recommendations

### 8.1 Priority Matrix

Effort vs. Impact for onboarding improvements:

| Recommendation | Effort | Impact | Priority |
|---------------|--------|--------|----------|
| Auto-generate in `fmm init` | Low | High | **P0** |
| Show sample sidecar after generate | Low | High | **P0** |
| Suggest real search command post-init | Low | Medium | **P0** |
| Record asciinema demo | Low | High | **P1** |
| Convert to animated SVG for README | Low | High | **P1** |
| Publish to crates.io | Low | High | **P1** |
| Pre-built binaries (GitHub Releases) | Medium | High | **P1** |
| Homebrew formula | Medium | Medium | **P2** |
| Better error messages with recovery actions | Medium | Medium | **P2** |
| `examples/demo-project/` | Medium | Medium | **P2** |
| README restructure (hero visual, badges) | Low | Medium | **P2** |
| `fmm watch` mode | High | Medium | **P3** |
| Team setup guide | Medium | Low | **P3** |

### 8.2 Immediate Actions (This Sprint)

1. **Modify `fmm init` to auto-run `generate`** after creating config files. Add a `--no-generate` flag for those who want the old behavior.

2. **After generate, print one example sidecar** and a suggested search command using a real export from the project.

3. **Add recovery suggestions to all error paths**: every `bail!()` and error print should include "Run `fmm [command]` to fix."

4. **Record an asciinema demo** of the improved init flow on a real project.

### 8.3 Next Sprint

5. **Publish to crates.io** so `cargo install fmm` works without cloning.

6. **Add GitHub Releases** with pre-built binaries for macOS (ARM + Intel), Linux (x86_64), and Windows.

7. **Restructure README**: hero visual, one-line install, before/after comparison, then details.

8. **Create `examples/demo-project/`** with walkthrough.

### 8.4 Future

9. **`fmm watch`** — auto-update sidecars on file change (inotify/fsevents).

10. **Homebrew formula** — `brew install fmm`.

11. **VS Code extension** — sidecar preview in editor sidebar.

12. **Onboarding analytics** — track crates.io downloads, GitHub clone events, and `.mcp.json` references in public repos to measure funnel health.

---

## Sources

### CLI Onboarding & First-Run Experience
- [Deno init documentation](https://docs.deno.com/runtime/reference/cli/init/)
- [create-next-app documentation](https://www.npmjs.com/package/create-next-app)
- [gh auth login manual](https://cli.github.com/manual/gh_auth_login)
- [Wrangler CLI commands](https://developers.cloudflare.com/workers/wrangler/commands/)
- [Cloudflare Workers getting started](https://developers.cloudflare.com/workers/get-started/guide/)

### Error Messages
- [Elm: Compiler errors for humans](https://elm-lang.org/news/compiler-errors-for-humans)
- [Rust diagnostics development guide](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
- [Rustacean Principles: Improving compiler errors](https://rustacean-principles.netlify.app/how_to_rustacean/bring_joy/improving_compiler_error.html)
- [RFC 1644: Default and expanded rustc errors](https://rust-lang.github.io/rfcs/1644-default-and-expanded-rustc-errors.html)
- [Writing Good Compiler Error Messages (Caleb Mer)](https://calebmer.com/2019/07/01/writing-good-compiler-error-messages.html)

### README Design
- [awesome-readme curated list](https://github.com/matiassingers/awesome-readme)
- [How to write a good README](https://dev.to/merlos/how-to-write-a-good-readme-bog)
- [Make a README](https://www.makeareadme.com/)
- [Crafting effective README for open-source](https://www.gitdevtool.com/blog/readme-best-practice)
- [Make your README stand out with animated GIFs/SVGs](https://dev.to/brpaz/make-your-project-readme-file-stand-out-with-animated-gifs-svgs-4kpe)

### Interactive Demos
- [asciinema documentation](https://docs.asciinema.org/)
- [asciinema automation](https://github.com/PierreMarchand20/asciinema_automation)
- [Instruqt (Katacoda alternative)](https://instruqt.com/katacoda-comparison)

### Activation & Metrics
- [Time to First Value in SaaS](https://payproglobal.com/answers/what-is-saas-time-to-first-value-ttfv/)
- [TTFV as a customer onboarding goal](https://sixteenventures.com/customer-onboarding-ttfv)
- [User activation in SaaS](https://usermaven.com/blog/user-activation)
- [Product adoption funnel](https://uxcam.com/blog/product-adoption-funnel/)
- [Funnel analysis guide](https://usermaven.com/blog/funnel-analysis)

### Progressive Disclosure
- [Nielsen Norman Group: Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [Interaction Design Foundation: Progressive Disclosure](https://www.interaction-design.org/literature/topics/progressive-disclosure)
- [Claude Code progressive disclosure analysis](https://medium.com/@quanap5/claude-code-progressive-disclosure-insights-from-my-learning-5244bc9864aa)
- [Progressive disclosure examples in SaaS](https://userpilot.com/blog/progressive-disclosure-examples/)
