# Documentation Site Research for fmm

> Research date: 2025-01-31
> Purpose: Inform the documentation strategy for fmm (Frontmatter Matters), a Rust CLI tool for structured code metadata via `.fmm` sidecar files.

---

## Table of Contents

1. [World-Class Documentation Sites Analyzed](#1-world-class-documentation-sites-analyzed)
2. [Documentation Frameworks Compared](#2-documentation-frameworks-compared)
3. [Content Architecture for CLI Tools](#3-content-architecture-for-cli-tools)
4. [What Makes Docs World-Class](#4-what-makes-docs-world-class)
5. [Docs-as-Code: Keeping Docs in Sync](#5-docs-as-code-keeping-docs-in-sync)
6. [AI-Ready Documentation](#6-ai-ready-documentation)
7. [Recommendation for fmm](#7-recommendation-for-fmm)

---

## 1. World-Class Documentation Sites Analyzed

### Stripe API Docs — The Gold Standard

**URL**: https://docs.stripe.com/api

**What they do right:**
- **Three-column layout**: Navigation left, content center, code examples right. Readers never lose context.
- **Language switcher**: Code examples in curl, Ruby, Python, PHP, Java, Node, Go, .NET. One click to change all examples on the page.
- **Copy-pasteable examples**: Every code block has a copy button. Examples use real (test-mode) API keys when authenticated.
- **Progressive disclosure**: High-level overview first, then endpoint details, then edge cases. Expandable sections for advanced parameters.
- **Authentication shown in context**: API keys appear inline in examples so you can copy-paste and run immediately.
- **REST-first organization**: Resources as top-level nav items (Customers, Charges, Subscriptions). Developers find what they need by the noun they're working with.

**Relevance to fmm**: The three-column pattern works when you have a command on the left, explanation in the center, and real terminal output on the right. fmm's CLI reference could show `fmm extract src/lib.rs` alongside the actual YAML output.

---

### Tailwind CSS — Best-in-Class Searchable Docs

**URL**: https://tailwindcss.com/docs

**What they do right:**
- **Command palette search** (Cmd+K): Instant, fuzzy search across all utilities. This is the single most-used feature.
- **Sticky sidebar with auto-scroll**: Current section highlighted, collapsible categories for 100+ utility pages.
- **Code blocks with file labels**: Each example shows exactly which file it belongs to (e.g., `vite.config.ts`, `Terminal`).
- **Framework-specific installation guides**: Not one install page but separate paths for Vite, PostCSS, CLI, CDN. Decision trees guide users.
- **Copy-paste-and-run examples**: Every installation step is numbered with terminal commands that work verbatim.
- **"Are you stuck?" callouts**: Proactive help links when users might be confused.
- **Dark mode as default**: Respects system preference with manual toggle.

**Relevance to fmm**: The Cmd+K search pattern is table stakes. fmm docs should support it. Framework-specific install guides map well to fmm's multiple install paths (cargo install, homebrew, binary download).

---

### The Rust Book — Progressive Tutorial + Reference

**URL**: https://doc.rust-lang.org/book/

**What they do right:**
- **Sequential chapter structure**: Builds mental models incrementally. Chapter 1 doesn't assume Chapter 3 knowledge.
- **Interactive variant**: The Brown University edition adds quizzes, highlighting, and memory diagrams.
- **Multiple formats**: HTML, EPUB, paperback. `rustup doc --book` for offline access.
- **Built with mdBook**: Rust ecosystem native. `mdbook test` validates all code examples compile.
- **Edition-aware**: Documents edition-specific behavior (2021, 2024) with appendix explaining the system.
- **Theme choices**: Light, Rust, Coal, Navy, Ayu. Developers spend hours here; comfort matters.

**Relevance to fmm**: mdBook is the natural framework choice for a Rust project. The progressive chapter structure is the right model for teaching the fmm mental model (what are sidecars? why? how?). Code example testing via `mdbook test` prevents doc rot.

---

### Deno — Clean, Modern Docs

**URL**: https://docs.deno.com

**What they do right:**
- **Hierarchical sidebar**: Getting Started > Fundamentals > Reference Guides > Contributing. Clear audience targeting.
- **Command palette** (Cmd+K): Fast search with keyboard-first navigation.
- **Skip-to-content links**: Accessibility-first design.
- **Dark/light mode** with system preference detection and localStorage persistence.
- **Ecosystem-aware footer**: Links to Deploy, Subhosting, Discord, GitHub. Docs as gateway to the full platform.

**Relevance to fmm**: The four-tier hierarchy (Getting Started / Fundamentals / Reference / Contributing) is a clean model for fmm's doc structure.

---

### Astro / Starlight — Docs-as-Code Excellence

**URL**: https://docs.astro.build (powered by Starlight)

**What they do right:**
- **Built on Starlight**: Astro's own doc framework. Dogfooding at its finest.
- **Frontmatter validation**: TypeScript type-safe frontmatter in docs. (Ironic relevance to fmm.)
- **i18n built in**: Community translations without bolted-on solutions.
- **Component islands in docs**: Interactive React/Vue/Svelte components embedded in Markdown pages.
- **Automatic table of contents**: Generated from headings, sticky on the right.
- **Search via Pagefind**: Static search index, no server needed, works offline.

**Relevance to fmm**: Starlight is the most feature-rich option if we want interactive examples. However, it requires Node.js, which creates friction for a Rust-only project.

---

### Supabase — Developer-First Docs

**URL**: https://supabase.com/docs

**What they do right:**
- **Hub-and-spoke model**: Landing page with clear paths: Quickstarts, Products, Client Libraries, Migration Guides.
- **Framework-specific quickstarts**: React, Next.js, Vue, Flutter, Kotlin, Swift. Each is a separate guided path.
- **Migration guides from competitors**: Firebase, Heroku, AWS RDS. Meets developers where they are.
- **Multi-language client libraries**: JS, Python, C#, Swift, Kotlin, Flutter, each with dedicated reference docs.
- **Command palette search** (Cmd+K): Universal across all doc sections.

**Relevance to fmm**: The migration guide pattern could map to "Coming from X" pages (e.g., "Coming from JSDoc", "Coming from TypeScript declarations"). The hub-and-spoke landing page is a good model for fmm's doc homepage.

---

### Next.js — Interactive Examples

**URL**: https://nextjs.org/docs

**What they do right:**
- **App Router vs Pages Router toggle**: Two parallel doc tracks for different architectures. One click to switch.
- **Interactive code playgrounds**: Embedded sandboxes where you can edit and run examples.
- **Breadcrumb navigation**: Always know where you are in the hierarchy.
- **Versioned docs**: Each major release maintains its own documentation.
- **"Good to know" callouts**: Highlighted boxes for gotchas and best practices.

**Relevance to fmm**: The parallel documentation track idea could apply if fmm has both CLI and MCP modes. The "Good to know" callout pattern is universally useful.

---

## 2. Documentation Frameworks Compared

### Framework Comparison Matrix

| Framework | Language | Content Format | Search | Dark Mode | i18n | Versioning | Best For |
|-----------|----------|---------------|--------|-----------|------|------------|----------|
| **mdBook** | Rust | Markdown | Built-in | Themes (5) | Limited | Manual | Rust ecosystem projects |
| **Starlight** | JS (Astro) | MD/MDX | Pagefind | Built-in | Built-in | Plugin | Feature-rich doc sites |
| **Docusaurus** | JS (React) | MD/MDX | Algolia | Built-in | Built-in | Built-in | Large open-source projects |
| **MkDocs Material** | Python | Markdown | Built-in (offline) | Built-in | 60+ languages | Plugin | Beautiful technical docs |
| **VitePress** | JS (Vue) | MD | Built-in | Built-in | Manual | Manual | Vue ecosystem, fast sites |
| **Mintlify** | SaaS | MD/MDX | Built-in | Built-in | Partial | Built-in | API docs, AI-native |

### Detailed Assessment

#### mdBook (Recommended for fmm)

**Source**: https://rust-lang.github.io/mdBook/

- **Rust-native**: `cargo install mdbook`. No Node.js, no Python, no external runtime.
- **Used by**: The Rust Book, Rust CLI Book, Rust by Example, Rust Reference. The Rust ecosystem standard.
- **Code testing**: `mdbook test` compiles and runs Rust code blocks in documentation. Guarantees examples work.
- **Preprocessor system**: Extensible via Rust preprocessors for custom syntax.
- **Output formats**: HTML (primary), EPUB.
- **Search**: Built-in client-side search with elasticlunr.js.
- **Themes**: 5 built-in themes (Light, Rust, Coal, Navy, Ayu).
- **Limitations**: No i18n, no MDX, no component islands, basic layout (single-column with sidebar).

**Why it's right for fmm**: Zero additional dependencies. The entire toolchain stays Rust. Code examples are automatically tested. It's what the Rust community expects. The limitations (no MDX, basic layout) don't matter for a CLI tool's docs.

#### Starlight (Strong alternative)

**Source**: https://starlight.astro.build

- **Full-featured**: Search (Pagefind), i18n, dark mode, sidebar generation, TypeScript frontmatter validation.
- **Component islands**: Embed interactive React/Vue/Svelte demos inside Markdown.
- **Accessibility**: WCAG compliance built in. Semantic HTML, keyboard navigation.
- **Performance**: Astro's zero-JS-by-default architecture. Static HTML with islands of interactivity.
- **Trade-off**: Requires Node.js. Adds a `docs/` project with `package.json`, `node_modules/`, etc.

**When to choose Starlight over mdBook**: If fmm ever needs interactive demos (e.g., a live playground for trying fmm commands), Starlight is the upgrade path.

#### Mintlify (Worth watching)

**Source**: https://mintlify.com

- **AI-native**: Built for the age of LLMs. Auto-generates `llms.txt` and supports MCP.
- **SaaS model**: No self-hosting. Push Markdown, Mintlify deploys.
- **Beautiful defaults**: Polished out of the box without design effort.
- **Used by**: Anthropic, Cursor, and thousands of developer tools.
- **Trade-off**: Vendor lock-in. Pricing tiers. Less control.

**Relevance to fmm**: Mintlify's auto-generation of `llms.txt` is interesting given fmm's MCP integration. However, the SaaS model and vendor dependency make it a poor fit for an open-source Rust tool.

---

## 3. Content Architecture for CLI Tools

Based on analysis of the best CLI tool documentation, here is the recommended page structure for fmm:

### Tier 1: Get Running (Time to first value < 5 minutes)

```
docs/
  getting-started/
    index.md          # What is fmm? (30-second pitch)
    installation.md   # cargo install, brew, binary download
    quickstart.md     # First sidecar in 60 seconds
```

**Installation page must cover:**
- `cargo install fmm` (Rust developers)
- `brew install fmm` (macOS, when available)
- Binary downloads from GitHub releases (everyone else)
- Version verification: `fmm --version`

**Quickstart must demonstrate:**
1. Point fmm at an existing file: `fmm extract src/main.rs`
2. Show the output (real YAML, not hypothetical)
3. Show the generated sidecar file
4. Explain what just happened

### Tier 2: Understand (Mental model)

```
  concepts/
    what-are-sidecars.md    # The core idea
    metadata-model.md       # What fmm extracts and why
    navigation.md           # How sidecars enable code navigation
    mcp-integration.md      # fmm as an MCP tool
```

### Tier 3: Use (Task-oriented guides)

```
  guides/
    extracting-metadata.md    # Deep dive on fmm extract
    comparing-files.md        # fmm compare workflow
    ci-integration.md         # Running fmm in CI/CD
    editor-integration.md     # VS Code, Neovim, etc.
    large-codebases.md        # Performance, .fmmignore, parallelism
    custom-extractors.md      # Plugin/extension points
```

### Tier 4: Reference (Look up specifics)

```
  reference/
    cli.md              # Auto-generated from clap definitions
    configuration.md    # .fmm.toml / fmm.yaml reference
    sidecar-format.md   # .fmm file format specification
    mcp-tools.md        # MCP tool definitions and schemas
    environment.md      # Environment variables
```

### Tier 5: Ecosystem

```
  ecosystem/
    changelog.md         # What changed and when
    contributing.md      # How to contribute
    architecture.md      # How fmm works internally
    faq.md               # Common questions
    troubleshooting.md   # Common errors and fixes
```

### Content Principles

1. **Every page answers one question**. "How do I install fmm?" is one page. "How do I run fmm in CI?" is another.
2. **Show real output**. Never write "output will look something like..." — run the command and paste the actual output.
3. **Copy-paste-and-run**. Every command block should work if pasted verbatim into a terminal.
4. **Progressive disclosure**. Quickstart shows 3 commands. Guides show 20. Reference shows everything.

---

## 4. What Makes Docs World-Class

### The Non-Negotiables

| Feature | Why It Matters | Implementation |
|---------|---------------|----------------|
| **Copy button on code blocks** | Reduces friction by 80%. Developers copy-paste constantly. | mdBook has this built in. |
| **Real output shown** | Builds trust. Hypothetical output signals "this might not work." | Generate output as part of doc build. |
| **Search that works** | Developers don't browse docs — they search. If search fails, they leave. | mdBook's elasticlunr.js or add Pagefind. |
| **Dark mode** | Developers work in dark mode. Bright-white docs cause physical pain. | mdBook themes: Coal, Navy, Ayu. |
| **Mobile-responsive** | Many developers read docs on phones while coding on desktop. | mdBook handles this. |

### The Differentiators

| Feature | Why It Matters | Implementation |
|---------|---------------|----------------|
| **Cmd+K command palette** | Power users expect it. Faster than sidebar browsing. | Custom JS in mdBook, or switch to Starlight. |
| **Version-specific docs** | Users on v0.3 shouldn't see v0.5 features. | Git tags + branch-based builds. |
| **Changelog with migration guides** | People upgrade when they know what changed and what breaks. | `CHANGELOG.md` rendered as a doc page. |
| **"Edit this page" links** | Lowers the barrier to community contributions. | mdBook supports this via `git-repository-url`. |
| **Time-to-read estimates** | Sets expectations. Developers budget their time. | Custom preprocessor or manual. |
| **Breadcrumbs** | "Where am I?" is the #1 navigation question. | Custom mdBook theme or Starlight. |

### Content Quality Markers

**Good documentation reads like this:**
```
## Extract metadata from a file

$ fmm extract src/auth.rs

This generates `src/auth.rs.fmm` containing:

---
exports:
  - name: authenticate
    kind: function
    line: 12
  - name: AuthConfig
    kind: struct
    line: 45
imports:
  - jsonwebtoken
  - serde
loc: 89
---
```

**Bad documentation reads like this:**
```
## Extraction

The extract command extracts metadata from source files. It supports
various languages and output formats. See the configuration section
for more details on customization options.
```

The difference: the good version shows a real command with real output. The bad version describes what something does without demonstrating it.

---

## 5. Docs-as-Code: Keeping Docs in Sync

### Auto-Generating CLI Reference from Clap

fmm uses clap for argument parsing. Three tools auto-generate docs from clap definitions:

#### clap-markdown (Recommended)

**Source**: https://lib.rs/crates/clap-markdown

Generates Markdown from `clap::Command` definitions. Usage:

```rust
// In a binary or xtask:
use clap::CommandFactory;
use clap_markdown;

let markdown = clap_markdown::help_markdown::<Cli>();
std::fs::write("docs/reference/cli.md", markdown)?;
```

**Workflow**:
1. Add `--markdown-help` hidden flag to fmm
2. CI runs `fmm --markdown-help > docs/reference/cli.md`
3. CI checks if the file changed (if so, docs are stale)
4. Alternatively, use a `just` task: `just generate-cli-docs`

#### clap_mangen (For man pages)

**Source**: https://crates.io/crates/clap_mangen

Generates ROFF man pages from clap definitions. Used in `build.rs`:

```rust
use clap::CommandFactory;
use clap_mangen::Man;

let cmd = Cli::command();
let man = Man::new(cmd);
let mut buffer = Vec::new();
man.render(&mut buffer)?;
std::fs::write("target/man/fmm.1", buffer)?;
```

**Ship both**: Markdown for the website, man pages for `man fmm` on Unix systems.

#### clap_complete (Shell completions as docs)

Shell completions are a form of documentation. Auto-generate them:

```rust
use clap_complete::{generate, Shell};
generate(Shell::Bash, &mut cmd, "fmm", &mut std::io::stdout());
```

### Testing Code Examples in Documentation

#### mdBook's built-in testing

mdBook compiles and runs Rust code blocks:

```bash
mdbook test
```

Any fenced Rust code block in the docs gets compiled. If it doesn't compile, the test fails. This is the single most powerful docs-as-code feature for a Rust project.

#### Testing shell command examples

For non-Rust code blocks (shell commands), use a custom test harness:

```bash
# Extract all shell commands from docs and verify they parse
grep -h '^\$ fmm' docs/src/**/*.md | sed 's/^\$ //' | while read cmd; do
  echo "Testing: $cmd"
  eval "$cmd --help" > /dev/null 2>&1 || echo "FAIL: $cmd"
done
```

### CI Checks for Doc Freshness

**Recommended CI pipeline:**

```yaml
# .github/workflows/docs.yml
name: Documentation
on: [push, pull_request]

jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install mdBook
        run: cargo install mdbook

      - name: Build docs
        run: mdbook build docs/

      - name: Test code examples
        run: mdbook test docs/

      - name: Check CLI reference freshness
        run: |
          cargo run -- --markdown-help > /tmp/cli-reference.md
          diff docs/src/reference/cli.md /tmp/cli-reference.md || {
            echo "CLI reference is stale. Run: just generate-cli-docs"
            exit 1
          }

      - name: Lint Markdown
        uses: DavidAnson/markdownlint-cli2-action@v14

      - name: Check links
        run: |
          cargo install mdbook-linkcheck
          mdbook build docs/  # linkcheck runs as a backend
```

### Keeping Docs in the Repo

```
fmm/
  docs/
    book.toml           # mdBook configuration
    src/
      SUMMARY.md        # Table of contents (mdBook requires this)
      getting-started/
      concepts/
      guides/
      reference/
      ecosystem/
```

**book.toml**:
```toml
[book]
title = "fmm — Frontmatter Matters"
authors = ["Stuart"]
language = "en"
src = "src"

[build]
build-dir = "book"

[output.html]
git-repository-url = "https://github.com/user/fmm"
edit-url-template = "https://github.com/user/fmm/edit/main/docs/src/{path}"
default-theme = "coal"
preferred-dark-theme = "coal"

[output.html.search]
enable = true
limit-results = 20
use-hierarchical-heading = true
```

---

## 6. AI-Ready Documentation

### llms.txt

**Source**: https://llmstxt.org/

The emerging standard for making documentation consumable by LLMs. A `/llms.txt` file provides a Markdown index of your documentation optimized for AI context windows.

**Why it matters for fmm**: fmm is an MCP tool. Its users are developers who use AI assistants. Those AI assistants need to understand fmm's documentation to help users effectively. Providing `llms.txt` creates a virtuous cycle: AI assistants can recommend fmm and explain how to use it accurately.

**Implementation**:

```markdown
# fmm - Frontmatter Matters

> Structured code metadata via .fmm sidecar files. Navigate codebases without reading source.

## Documentation

- [Getting Started](https://fmm.dev/getting-started/): Install fmm and generate your first sidecar file
- [What Are Sidecars](https://fmm.dev/concepts/sidecars/): The core concept — YAML metadata files alongside source files
- [CLI Reference](https://fmm.dev/reference/cli/): All commands, flags, and options
- [MCP Tools](https://fmm.dev/reference/mcp-tools/): Using fmm as an MCP server for AI assistants
- [Configuration](https://fmm.dev/reference/configuration/): .fmm.toml reference
- [Sidecar Format](https://fmm.dev/reference/sidecar-format/): The .fmm YAML specification
```

**Also generate `/llms-full.txt`**: Concatenate all doc pages into a single Markdown file. For a CLI tool's docs, this will fit comfortably within modern context windows (100K-200K tokens).

### MCP-Aware Documentation

Since fmm exposes MCP tools, the documentation should include:

1. **MCP tool schemas** in the reference section (JSON Schema for each tool)
2. **Example MCP conversations** showing how an AI assistant uses fmm tools
3. **Integration guides** for Claude Desktop, Cursor, and other MCP clients

---

## 7. Recommendation for fmm

### Framework: mdBook

**Decision**: Use mdBook as the documentation framework.

**Rationale**:
- Zero non-Rust dependencies. `cargo install mdbook` and done.
- Built-in code testing via `mdbook test` — the killer feature for keeping Rust examples correct.
- Used by the Rust Book, Rust CLI Book, and most Rust ecosystem projects. Users know the UX.
- Coal/Navy themes for dark mode. Built-in search. "Edit this page" links.
- If fmm ever outgrows mdBook, migration to Starlight is straightforward (Markdown content transfers 1:1).

**Enhancements to add**:
- `mdbook-linkcheck` — validates all links on build
- `mdbook-toc` — auto-generated table of contents in pages
- Custom CSS for callout boxes (tip, warning, note)

### Content Plan

**Phase 1 — Ship with v1.0** (minimum viable docs):
1. `getting-started/installation.md` — All install methods
2. `getting-started/quickstart.md` — First sidecar in 60 seconds
3. `reference/cli.md` — Auto-generated from clap via clap-markdown
4. `reference/sidecar-format.md` — The .fmm YAML spec
5. `reference/configuration.md` — .fmm.toml reference
6. `llms.txt` — AI-ready index

**Phase 2 — Post-launch** (build based on user questions):
1. `concepts/what-are-sidecars.md` — The mental model
2. `concepts/navigation.md` — How sidecars replace source reads
3. `guides/ci-integration.md` — Running fmm in CI
4. `guides/large-codebases.md` — Performance at scale
5. `reference/mcp-tools.md` — MCP tool reference

**Phase 3 — Community growth**:
1. `ecosystem/contributing.md`
2. `ecosystem/changelog.md`
3. `ecosystem/faq.md`
4. Tutorials for specific ecosystems (Rust, TypeScript, Python)

### Auto-Generation Pipeline

```
just generate-docs:
  1. cargo run -- --markdown-help > docs/src/reference/cli.md
  2. Generate man page via clap_mangen
  3. Generate llms.txt from SUMMARY.md
  4. mdbook build docs/
  5. mdbook test docs/
```

### Hosting

**GitHub Pages** via GitHub Actions. Free, no infrastructure, deploys on push to main.

```yaml
- name: Deploy to GitHub Pages
  uses: peaceiris/actions-gh-pages@v3
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./docs/book
```

### Quality Checklist

Every documentation page must:

- [ ] Answer exactly one question (stated in the H1)
- [ ] Show at least one copy-pasteable command
- [ ] Show real output (generated, not hand-written)
- [ ] Link to the next logical page ("Next: ...")
- [ ] Have been tested via `mdbook test` (for Rust examples)
- [ ] Appear in `SUMMARY.md` (or it won't be built)

---

## Sources

### Documentation Sites
- [Stripe API Docs](https://docs.stripe.com/api)
- [Tailwind CSS Docs](https://tailwindcss.com/docs)
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Deno Docs](https://docs.deno.com)
- [Astro Docs (Starlight)](https://docs.astro.build)
- [Next.js Docs](https://nextjs.org/docs)
- [Supabase Docs](https://supabase.com/docs)

### Documentation Frameworks
- [mdBook](https://rust-lang.github.io/mdBook/)
- [Starlight](https://starlight.astro.build)
- [Docusaurus](https://docusaurus.io)
- [MkDocs Material](https://squidfunk.github.io/mkdocs-material/)
- [VitePress](https://vitepress.dev)
- [Mintlify](https://mintlify.com)

### Rust CLI Documentation Tools
- [clap-markdown](https://lib.rs/crates/clap-markdown) — Generate Markdown from clap definitions
- [clap_mangen](https://crates.io/crates/clap_mangen) — Generate man pages from clap definitions
- [Command Line Apps in Rust — Docs chapter](https://rust-cli.github.io/book/in-depth/docs.html)

### Docs-as-Code
- [CI/CD and Docs-as-Code workflow (Pronovix)](https://pronovix.com/blog/cicd-and-docs-code-workflow)
- [How to test docs code examples (Dachary Carey)](https://dacharycarey.com/2024/01/12/how-to-test-docs-code-examples/)
- [Squarespace Docs-as-Code Journey](https://engineering.squarespace.com/blog/2025/making-documentation-simpler-and-practical-our-docs-as-code-journey)
- [Docs as Tests](https://www.docsastests.com/docs-as-tests-vs-docs-as-code)

### AI-Ready Documentation
- [llms.txt specification](https://llmstxt.org/)
- [Mintlify llms.txt support](https://www.mintlify.com/docs/ai/llmstxt)
- [What is llms.txt? (Bluehost explainer)](https://www.bluehost.com/blog/what-is-llms-txt/)
