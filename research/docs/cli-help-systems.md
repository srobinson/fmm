# CLI Help Systems Research

Research into world-class CLI help and documentation experiences, with concrete
recommendations for fmm.

---

## 1. The Two-Tier Help Pattern (`-h` vs `--help`)

The single most impactful pattern across elite CLIs is **differentiated short/long help**.

### ripgrep (the gold standard)

ripgrep deliberately made `-h` and `--help` produce different output after
[GitHub issue #189](https://github.com/BurntSushi/ripgrep/issues/189), where
users complained that help output scrolled useful info off screen.

**`rg -h` (condensed):**
- One line per flag, brief description only
- Fits in a single terminal screen
- Quick-reference for users who know the tool

**`rg --help` (verbose):**
- Full documentation per flag (multi-line)
- Equivalent to man page content
- Complete reference for deep dives

**`man rg` (full manual):**
- Includes prelude, detailed sections, cross-references
- Generated via `rg --generate man`

This three-tier approach (quick ref / full help / man page) is the benchmark.

### fd (same pattern, clean execution)

fd follows the same pattern: `fd -h` shows one-liner descriptions, `fd --help`
shows full docs. Additional design wins:
- Colorized output by default
- Smart defaults reduce flag count users need to learn
- Intuitive syntax means less reliance on help

### How clap 4 supports this

clap 4 natively supports this pattern with separate short/long variants:

```rust
#[derive(Parser)]
#[command(
    about = "Short description for -h",
    long_about = "Detailed description shown with --help.\n\
                   Can span multiple lines with full context."
)]
struct Cli { ... }
```

For individual arguments:

```rust
#[arg(
    short, long,
    help = "Brief flag description",           // shown with -h
    long_help = "Detailed explanation of this flag\n\
                  with examples and caveats."   // shown with --help
)]
```

---

## 2. Help Output Structure (What Goes Where)

### The winning structure (synthesized from kubectl, gh, cargo, rg)

```
{name} {version}
{about}

Usage: {usage}

Commands:            <-- or "Subcommands:"
  cmd1    Brief description
  cmd2    Brief description

Options:
  -f, --flag <VALUE>    Brief description [default: x]
  -v, --verbose         Brief description

Examples:
  $ tool do-thing --flag value
  $ tool other-thing path/

Learn more: https://tool.dev/docs
```

### Breakdown by section

| Section | Purpose | Who does it best |
|---------|---------|-----------------|
| Name + version | Identity, confirms correct binary | cargo, rg |
| One-line about | Immediate "what is this?" answer | fd, gh |
| Usage line | Syntax pattern | kubectl, rg |
| Commands list | Scannable subcommand overview | gh, docker |
| Options | Flag reference | rg (two-tier) |
| Examples | Show, don't tell | kubectl, cargo |
| Learn more | Escape hatch to docs | gh, clig.dev recommendation |

### gh (GitHub CLI) -- grouping by category

gh groups commands into semantic categories, which is effective when you have
many subcommands:

```
CORE COMMANDS
  auth:       Authenticate gh and git with GitHub
  browse:     Open the repository in the browser
  issue:      Manage issues
  pr:         Manage pull requests

ADDITIONAL COMMANDS
  alias:      Create command shortcuts
  api:        Make an authenticated GitHub API request
```

### kubectl -- examples-first per subcommand

kubectl's per-command help leads with a description, then shows 5-9 real-world
examples before listing flags. This is highly effective because users learn
faster from examples than from flag descriptions.

```
Display one or many resources.

Examples:
  # List all pods in ps output format
  kubectl get pods

  # List all pods with more information
  kubectl get pods -o wide

  # List a single pod in JSON format
  kubectl get -o json pod web-pod-13je7

Options:
  --all-namespaces=false: ...
  -f, --filename=[]: ...
  -o, --output='': ...
```

### docker -- layered discovery

docker uses a hierarchical help system:
- `docker --help` shows top-level command groups
- `docker container --help` shows container subcommands
- `docker container run --help` shows full flag details

Invalid plugins get a separate "Invalid plugins" section rather than silently
failing. Unknown commands attempt plugin discovery before failing.

---

## 3. Examples in Help Output

### Why examples matter

From [clig.dev](https://clig.dev/): "Lead with examples -- users prefer them
over other documentation forms." The [BetterCLI guide](https://bettercli.org/design/cli-help-page/)
confirms: help pages must answer "Where to begin?" with sample commands.

### Where to put examples

| Placement | Pros | Cons |
|-----------|------|------|
| Before flags (kubectl style) | Users see them first | Pushes flags down |
| After flags (cargo/rg style) | Clean separation | Users may not scroll |
| Both (key examples up top, full list at bottom) | Best of both | Longer output |

### Recommendation for fmm

Put 1-2 examples in `before_help` or at the top of `long_about`, and a full
"Examples" section in `after_long_help`. This way:
- `-h` stays clean (no examples clutter)
- `--help` shows examples prominently

---

## 4. Error Messages and "Did You Mean?"

### Best practices (from clig.dev, BetterCLI, and git's example)

1. **Error first, suggestion second:**
   ```
   error: 'staus' is not a fmm command. Did you mean 'status'?
   ```

2. **Use fuzzy matching**, not prefix matching. Levenshtein distance or
   `strsim` crate in Rust.

3. **Always include an escape hatch:**
   ```
   Run 'fmm --help' for a list of available commands.
   ```

4. **Actionable error messages:**
   Bad:  `Error: no sidecars found`
   Good: `No .fmm sidecars found. Run 'fmm generate' to create them.`

### clap built-in support

clap 4 already provides "did you mean?" suggestions for subcommand typos
out of the box. It will suggest the closest match automatically. For unknown
flags, clap suggests similar flag names.

---

## 5. Colored Output and Formatting

### What the best tools do

- **rg, fd, bat**: Color by default when stdout is a TTY, plain when piped
- **gh**: Bold for headers, dim for secondary info
- **cargo**: Yellow for warnings, red for errors, green for success

### Standards to follow

From [clig.dev](https://clig.dev/):
- Disable color when: `NO_COLOR` env var is set, `TERM=dumb`, stdout is not a TTY,
  or `--no-color` flag is passed
- Use color intentionally for highlighting, not decoration
- Red = error, yellow = warning, green = success, dim/gray = secondary

### clap 4 native styling

clap 4 styles help output by default. Custom styling is possible:

```rust
use clap::builder::styling::*;

let styles = Styles::styled()
    .header(AnsiColor::Green.on_default() | Effects::BOLD)
    .usage(AnsiColor::Green.on_default() | Effects::BOLD)
    .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
    .placeholder(AnsiColor::Cyan.on_default());

Command::new("fmm").styles(styles)
```

---

## 6. clap 4 Help Customization (fmm-specific)

fmm uses `clap 4.5` with derive macros. Here are the specific APIs available.

### Template tags for `help_template`

```
{name}              -- command name
{version}           -- version string
{author}            -- author (not in default template)
{about}             -- short description
{usage-heading}     -- "Usage:" label
{usage}             -- auto-generated usage string
{all-args}          -- all arguments, options, subcommands with headings
{options}           -- just options
{positionals}       -- just positional args
{subcommands}       -- just subcommands
{before-help}       -- content from before_help/before_long_help
{after-help}        -- content from after_help/after_long_help
{tab}               -- tab character
```

### Key methods on Command (via derive attributes)

```rust
#[derive(Parser)]
#[command(
    name = "fmm",
    version,
    about = "Short description for -h",
    long_about = "Longer description for --help",
    before_help = "Text before everything",
    before_long_help = "Text before everything (--help only)",
    after_help = "Text after everything (both -h and --help)",
    after_long_help = "Text after everything (--help only)",
    help_template = "...",      // override entire layout
    arg_required_else_help = true,  // show help when no args given
)]
```

### Adding a styled Examples section

The clap maintainers recommend `after_help` / `after_long_help` for examples,
since formatting varies too much for built-in support.

Using the `color-print` crate for ANSI styling:

```rust
use color_print::cstr;

const EXAMPLES: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>

  <dim>$</dim> <bold>fmm generate</bold>
    Generate sidecars for all source files in the current directory

  <dim>$</dim> <bold>fmm generate src/</bold>
    Generate sidecars for a specific directory

  <dim>$</dim> <bold>fmm search --export createStore</bold>
    Find which file exports a symbol

  <dim>$</dim> <bold>fmm search --loc ">500"</bold>
    Find large files (over 500 lines)

  <dim>$</dim> <bold>fmm validate</bold>
    Check all sidecars are up to date (great for CI)

  <dim>$</dim> <bold>fmm init</bold>
    Set up config, Claude skill, and MCP server
"#);

#[derive(Parser)]
#[command(
    name = "fmm",
    about = "Auto-generate code metadata sidecars for LLM navigation",
    long_about = "Frontmatter Matters (fmm) generates .fmm sidecar files alongside your \
                   source code. These YAML metadata files describe each file's exports, \
                   imports, and dependencies -- enabling LLM agents to navigate codebases \
                   without reading every source file.",
    after_long_help = EXAMPLES,
    arg_required_else_help = true,
    version,
)]
pub struct Cli { ... }
```

### The `clap-help` crate (alternative renderer)

The [clap-help](https://crates.io/crates/clap-help) crate provides:
- Width-aware table rendering for arguments
- Custom templates per section (title, usage, options, positionals)
- ANSI color customization with `termimad`
- More readable formatting than clap's default

Worth evaluating if the default clap output feels too generic.

### Per-subcommand examples

Each subcommand can have its own `after_long_help`:

```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Generate .fmm sidecar files for source files
    #[command(
        long_about = "Generate .fmm sidecar files for source files that don't have them. \
                       Existing sidecars are left untouched (use 'update' to refresh them).",
        after_long_help = "Examples:\n  $ fmm generate\n  $ fmm generate src/\n  $ fmm generate -n"
    )]
    Generate { ... },
}
```

---

## 7. Man Pages vs `--help` vs Web Docs

### When each is appropriate

| Format | Use case | Audience |
|--------|----------|----------|
| `-h` | Quick reference while working | Experienced users |
| `--help` | Learning a command, exploring options | All users |
| `man` page | Complete reference, searchable | Power users, offline |
| Web docs | Tutorials, guides, searchable, shareable | New users, sharing |

### Generating man pages from clap

clap can generate man pages at build time:

```rust
// build.rs
use clap_mangen::Man;

fn main() {
    let cmd = fmm::cli::Cli::command();
    let man = Man::new(cmd);
    let mut buffer = Vec::new();
    man.render(&mut buffer).unwrap();
    std::fs::write("fmm.1", buffer).unwrap();
}
```

Add to `Cargo.toml`:
```toml
[build-dependencies]
clap_mangen = "0.2"
```

### Recommendation for fmm

Start with excellent `--help` output (the 80% case). Add man page generation
later if the tool gains traction. Web docs are unnecessary at this stage --
the README and `--help` should be sufficient.

---

## 8. Contextual Help

### gh's approach

gh provides different help output based on context. For example, `gh pr`
shows different information when inside a git repo vs outside one.

### clig.dev recommendation

"Discoverable CLIs have comprehensive help texts, provide lots of examples,
suggest what command to run next, and suggest what to do when there is an error."

### Suggesting next steps

After each command, suggest what the user might want to do next:

```
$ fmm generate
Generated 42 sidecar(s)

Next: Run 'fmm validate' to verify, or 'fmm search --export <name>' to find symbols.
```

This is a pattern from Heroku CLI and modern tools like Claude Code's `/init`.

---

## 9. No-Args Behavior

### The `arg_required_else_help` pattern

From [clig.dev](https://clig.dev/): "Show help when no arguments are provided,
if the tool requires arguments."

In clap 4:
```rust
#[command(arg_required_else_help = true)]
```

This makes `fmm` (with no args) show help instead of an error. Essential for
discoverability.

---

## 10. Concrete Recommendations for fmm

### Priority 1: Quick wins (do now)

1. **Add `long_about`** to the main command with a clear explanation of what
   fmm does and why.

2. **Add `after_long_help`** with an Examples section showing the 5-6 most
   common workflows.

3. **Add `arg_required_else_help = true`** so bare `fmm` shows help.

4. **Add `long_about` to each subcommand** distinguishing the brief `-h`
   description from the detailed `--help` explanation.

### Priority 2: Polish (do next)

5. **Add per-subcommand examples** via `after_long_help` on each variant.

6. **Use `color-print` crate** to style the Examples heading to match clap's
   native heading style (bold + underline).

7. **Add actionable error messages** with "did you mean?" for common mistakes
   (clap does subcommand typos automatically; add custom messages for semantic
   errors like "no sidecars found").

8. **Group subcommands** into categories (Core: generate/update/validate/clean,
   Setup: init/status, Integration: mcp/serve/gh, Analysis: search/compare).
   Use `help_heading` in clap:

   ```rust
   #[derive(Subcommand)]
   pub enum Commands {
       /// Generate .fmm sidecar files
       #[command(help_heading = "Core Commands")]
       Generate { ... },

       /// Initialize fmm in this project
       #[command(help_heading = "Setup")]
       Init { ... },
   }
   ```

   Note: subcommand grouping via `help_heading` requires builder API in some
   clap versions. An alternative is to use `before_help` on the main command
   to show a categorized overview.

### Priority 3: Advanced (do later)

9. **Generate man pages** with `clap_mangen` in `build.rs`.

10. **Add shell completions** with `clap_complete`:
    ```rust
    Commands::Completions { shell } => {
        clap_complete::generate(shell, &mut Cli::command(), "fmm", &mut io::stdout());
    }
    ```

11. **Consider `clap-help` crate** for width-aware, branded help rendering
    if the default layout feels too generic.

---

## 11. Current fmm Help Audit

Current `fmm --help` output:

```
Frontmatter Matters - Auto-generate code metadata sidecars for LLM navigation

Usage: fmm <COMMAND>

Commands:
  generate  Generate .fmm sidecar files for source files that don't have them
  update    Update all .fmm sidecar files (regenerate from source)
  validate  Validate that .fmm sidecars are up to date
  clean     Remove all .fmm sidecar files (and legacy .fmm/ directory)
  init      Initialize fmm in this project (config, skill, MCP)
  status    Show current fmm status and configuration
  search    Search sidecars for files and exports
  mcp       Start MCP (Model Context Protocol) server for LLM integration
  serve     Start MCP server for LLM integration (alias for 'mcp')
  gh        GitHub integrations (issue fixing, PR creation)
  compare   Compare FMM vs control performance on a GitHub repository
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### What's good

- Clean, scannable list of commands
- Brief one-line descriptions
- Standard `-h`/`-V` flags

### What's missing

- No `long_about` (so `-h` and `--help` show identical output)
- No examples section
- No explanation of what sidecars are or why you'd want them
- No suggested workflow / getting started path
- No command grouping (11 commands in a flat list is hard to parse)
- No "Learn more" link
- Running bare `fmm` could show help instead of requiring `fmm --help`

---

## Sources

- [Command Line Interface Guidelines (clig.dev)](https://clig.dev/)
- [BetterCLI.org: CLI Help Pages](https://bettercli.org/design/cli-help-page/)
- [Atlassian: 10 Design Principles for Delightful CLIs](https://www.atlassian.com/blog/it-teams/10-design-principles-for-delightful-clis)
- [UX Patterns for CLI Tools (Lucas F. Costa)](https://lucasfcosta.com/2022/06/01/ux-patterns-cli-tools.html)
- [ripgrep issue #189: Short help design](https://github.com/BurntSushi/ripgrep/issues/189)
- [clap Command API docs](https://docs.rs/clap/latest/clap/struct.Command.html)
- [clap v4.2 blog post (epage)](https://epage.github.io/blog/2023/03/clap-v4-2/)
- [clap discussion #3725: Examples in help](https://github.com/clap-rs/clap/discussions/3725)
- [clap issue #4132: Polishing --help output](https://github.com/clap-rs/clap/issues/4132)
- [clap-help crate](https://crates.io/crates/clap-help)
- [GitHub CLI manual](https://cli.github.com/manual/)
- [kubectl reference docs](https://kubernetes.io/docs/reference/kubectl/)
- [fd GitHub repository](https://github.com/sharkdp/fd)
- [bat GitHub repository](https://github.com/sharkdp/bat)
- [Cargo: extending with custom commands](https://doc.rust-lang.org/book/ch14-05-extending-cargo.html)
- [Cargo PR #16432: nested command manpages](https://github.com/rust-lang/cargo/pull/16432)
- [Claude Code docs](https://code.claude.com/docs/en/overview)
