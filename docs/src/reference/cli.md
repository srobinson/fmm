# Command-Line Help for `fmm`

This document contains the help content for the `fmm` command-line program.

**Command Overview:**

* [`fmm`↴](#fmm)
* [`fmm generate`↴](#fmm-generate)
* [`fmm update`↴](#fmm-update)
* [`fmm validate`↴](#fmm-validate)
* [`fmm clean`↴](#fmm-clean)
* [`fmm init`↴](#fmm-init)
* [`fmm status`↴](#fmm-status)
* [`fmm search`↴](#fmm-search)
* [`fmm mcp`↴](#fmm-mcp)
* [`fmm completions`↴](#fmm-completions)
* [`fmm gh`↴](#fmm-gh)
* [`fmm gh issue`↴](#fmm-gh-issue)
* [`fmm gh batch`↴](#fmm-gh-batch)
* [`fmm compare`↴](#fmm-compare)

## `fmm`

Frontmatter Matters (fmm) generates .fmm sidecar files alongside your source code. Each sidecar is a small YAML file listing the exports, imports, dependencies, and line count of its companion source file.

LLM agents use these sidecars to navigate codebases without reading every source file — reducing token usage by 80-90% while maintaining full structural awareness.

Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C++, C#, Ruby

Core Commands
  generate      Create .fmm sidecar files for source files
  update        Regenerate all .fmm sidecars from source
  validate      Check sidecars are up to date (CI-friendly)
  clean         Remove all .fmm sidecar files

Setup
  init          Initialize fmm in this project (config, skill, MCP)
  status        Show current fmm status and configuration
  completions   Generate shell completions (bash, zsh, fish, powershell, elvish)

Integration
  mcp           Start MCP server for LLM tool integration
  gh            GitHub integrations (issue fixing, PR creation)

Analysis
  search        Query sidecars by export, import, dependency, or LOC
  compare       Benchmark FMM vs control on a GitHub repository


**Usage:** `fmm [COMMAND]`

Examples

  $ fmm init
    Set up config, Claude skill, and MCP server in one step

  $ fmm generate
    Create .fmm sidecars for all source files in the current directory

  $ fmm generate src/
    Generate sidecars for a specific directory only

  $ fmm search --export createStore
    Find which file defines a symbol (O(1) lookup via reverse index)

  $ fmm search --loc ">500"
    Find large files (over 500 lines)

  $ fmm validate
    Check all sidecars are current — great for CI pipelines

Learn more

  https://github.com/mdcontext/fmm

###### **Subcommands:**

* `generate` — Create .fmm sidecar files for source files
* `update` — Regenerate all .fmm sidecars from source
* `validate` — Check sidecars are up to date (CI-friendly)
* `clean` — Remove all .fmm sidecar files
* `init` — Initialize fmm in this project (config, skill, MCP)
* `status` — Show current fmm status and configuration
* `search` — Query sidecars by export, import, dependency, or LOC
* `mcp` — Start MCP server for LLM tool integration
* `completions` — Generate shell completions for bash, zsh, fish, or powershell
* `gh` — GitHub integrations (issue fixing, PR creation)
* `compare` — Benchmark FMM vs control on a GitHub repository



## `fmm generate`

Create .fmm sidecar files for source files that don't already have them.

Each sidecar captures the file's exports, imports, dependencies, and line count in a compact YAML format. Existing sidecars are left untouched — use 'update' to refresh them.

**Usage:** `fmm generate [OPTIONS] [PATH]`

Examples

  $ fmm generate
    Generate sidecars for all supported files in the current directory

  $ fmm generate src/
    Generate sidecars for a specific directory

  $ fmm generate -n
    Dry run — show what would be created without writing files

###### **Arguments:**

* `<PATH>` — Path to file or directory

  Default value: `.`

###### **Options:**

* `-n`, `--dry-run` — Show what would be created without writing files



## `fmm update`

Regenerate all .fmm sidecar files from their source files.

Unlike 'generate' which skips existing sidecars, 'update' overwrites every sidecar with fresh metadata. Use after refactoring or when sidecars may be stale.

**Usage:** `fmm update [OPTIONS] [PATH]`

Examples

  $ fmm update
    Refresh all sidecars in the current directory

  $ fmm update src/ -n
    Preview which sidecars would change

###### **Arguments:**

* `<PATH>` — Path to file or directory

  Default value: `.`

###### **Options:**

* `-n`, `--dry-run` — Show what would be changed without writing files



## `fmm validate`

Validate that all .fmm sidecars match their source files.

Returns exit code 0 if all sidecars are current, or 1 if any are stale or missing. Designed for CI pipelines — add to your pre-commit hooks or GitHub Actions.

**Usage:** `fmm validate [PATH]`

Examples

  $ fmm validate
    Check all sidecars in the current directory

  $ fmm validate src/
    Check a specific directory

###### **Arguments:**

* `<PATH>` — Path to file or directory

  Default value: `.`



## `fmm clean`

Remove all .fmm sidecar files and the legacy .fmm/ directory.

Use this to cleanly uninstall fmm from a project or to start fresh.

**Usage:** `fmm clean [OPTIONS] [PATH]`

Examples

  $ fmm clean
    Remove all sidecars in the current directory

  $ fmm clean -n
    Preview what would be removed

###### **Arguments:**

* `<PATH>` — Path to file or directory

  Default value: `.`

###### **Options:**

* `-n`, `--dry-run` — Show what would be removed without deleting files



## `fmm init`

Set up fmm in the current project.

Creates .fmmrc.json config, installs the Claude Code skill for sidecar-aware navigation, and configures the MCP server in .mcp.json. Run with no flags for the full setup, or use --skill/--mcp to install individual components.

**Usage:** `fmm init [OPTIONS]`

Examples

  $ fmm init
    Full setup — config, skill, and MCP server

  $ fmm init --skill
    Install only the Claude Code navigation skill

  $ fmm init --mcp
    Install only the MCP server configuration

###### **Options:**

* `--skill` — Install Claude Code skill only (.claude/skills/fmm-navigate.md)
* `--mcp` — Install MCP server config only (.mcp.json)
* `--all` — Install all integrations (non-interactive)
* `--no-generate` — Skip auto-generating sidecars (config files only)



## `fmm status`

Display the current fmm configuration, supported languages, and workspace statistics including source file and sidecar counts.

**Usage:** `fmm status`



## `fmm search`

Search sidecar metadata to find files by export name, import path, dependency, or line count.

Export lookups use a reverse index for O(1) performance. Filters can be combined. With no filters, lists all indexed files.

**Usage:** `fmm search [OPTIONS]`

Examples

  $ fmm search --export createStore
    Find which file defines 'createStore'

  $ fmm search --imports react
    Find all files that import from 'react'

  $ fmm search --loc ">500"
    Find files over 500 lines

  $ fmm search --depends-on src/utils.ts --json
    Find dependents of a file, output as JSON

###### **Options:**

* `-e`, `--export <EXPORT>` — Find file by export name (O(1) reverse-index lookup)
* `-i`, `--imports <IMPORTS>` — Find files that import a module
* `-l`, `--loc <LOC>` — Filter files by line count.

   Supports comparison operators: >500, <100, >=50, <=1000, =200.
   A bare number is treated as exact match (=).
* `-d`, `--depends-on <DEPENDS_ON>` — Find files that depend on a path
* `-j`, `--json` — Output as JSON



## `fmm mcp`

Start the Model Context Protocol (MCP) server over stdio.

The MCP server exposes fmm's search and metadata capabilities as tools that LLM agents (Claude, GPT, etc.) can call directly. Add to .mcp.json with 'fmm init --mcp'.

**Usage:** `fmm mcp`



## `fmm completions`

Generate shell completion scripts for fmm.

Outputs a completion script for the specified shell to stdout. Redirect to the appropriate file for your shell to enable tab completion.

**Usage:** `fmm completions <SHELL>`

Examples

  $ fmm completions bash > ~/.local/share/bash-completion/completions/fmm
  $ fmm completions zsh > ~/.zfunc/_fmm
  $ fmm completions fish > ~/.config/fish/completions/fmm.fish
  $ fmm completions powershell > _fmm.ps1

###### **Arguments:**

* `<SHELL>` — Target shell

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




## `fmm gh`

GitHub workflow integrations powered by fmm sidecar metadata.

Currently supports automated issue fixing: clone a repo, generate sidecars, extract code references from the issue, and invoke Claude with focused context to create a PR.

**Usage:** `fmm gh <COMMAND>`

###### **Subcommands:**

* `issue` — Fix a GitHub issue: clone, generate sidecars, invoke Claude, create PR
* `batch` — Run batch A/B comparisons across a corpus of GitHub issues



## `fmm gh issue`

Automated GitHub issue fixing powered by fmm.

Pipeline: parse issue URL → fetch issue details → clone repo → generate sidecars → extract code references → resolve against sidecar index → build focused prompt → create branch → invoke Claude → commit → push → create PR.

**Usage:** `fmm gh issue [OPTIONS] <URL>`

Examples

  $ fmm gh issue https://github.com/owner/repo/issues/42
    Fix an issue and create a PR

  $ fmm gh issue https://github.com/owner/repo/issues/42 -n
    Dry run — show extracted refs and assembled prompt

  $ fmm gh issue https://github.com/owner/repo/issues/42 --no-pr
    Fix and commit but skip PR creation

  $ fmm gh issue https://github.com/owner/repo/issues/42 --compare
    A/B comparison — run control vs fmm, output token savings report

###### **Arguments:**

* `<URL>` — GitHub issue URL (e.g., https://github.com/owner/repo/issues/123)

###### **Options:**

* `--model <MODEL>` — Claude model to use

  Default value: `sonnet`
* `--max-turns <MAX_TURNS>` — Maximum turns for Claude

  Default value: `30`
* `--max-budget <MAX_BUDGET>` — Maximum budget in USD

  Default value: `5.0`
* `-n`, `--dry-run` — Show plan without executing (extract refs + assembled prompt)
* `--branch-prefix <BRANCH_PREFIX>` — Git branch prefix

  Default value: `fmm`
* `--no-pr` — Commit and push only, skip PR creation
* `--workspace <WORKSPACE>` — Override workspace directory
* `--compare` — Run A/B comparison: control (no sidecars) vs fmm (with sidecars). Outputs a comparison report instead of creating a PR
* `--output <OUTPUT>` — Output directory for comparison report (only used with --compare)



## `fmm gh batch`

Run A/B comparisons (control vs fmm) across a corpus of GitHub issues.

Reads an issues.json corpus file, runs each issue through the compare pipeline, checkpoints progress for resume, and aggregates results into proof-dataset.json and proof-dataset.md.

**Usage:** `fmm gh batch [OPTIONS] <CORPUS>`

Examples

  $ fmm gh batch proofs/issues.json --dry-run
    Show plan + cost estimate without running

  $ fmm gh batch proofs/issues.json --output proofs/dataset/ --max-budget 100
    Run full corpus with $100 total budget

  $ fmm gh batch proofs/issues.json --output proofs/dataset/ --resume
    Resume a previous run, skipping completed issues

###### **Arguments:**

* `<CORPUS>` — Path to corpus file (issues.json)

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output directory for results and checkpoint

  Default value: `proofs/dataset`
* `--model <MODEL>` — Claude model to use

  Default value: `sonnet`
* `--max-turns <MAX_TURNS>` — Maximum turns per issue

  Default value: `30`
* `--max-budget <MAX_BUDGET>` — Maximum budget in USD (total across all issues)

  Default value: `100.0`
* `-n`, `--dry-run` — Show plan + cost estimate without executing
* `--resume` — Resume from checkpoint, skipping completed issues



## `fmm compare`

Run controlled comparisons of FMM-assisted vs unassisted Claude performance on a GitHub repository.

Clones the repo, generates sidecars, runs a set of coding tasks with and without FMM, and produces a report comparing token usage, cost, and quality.

**Usage:** `fmm compare [OPTIONS] <URL>`

Examples

  $ fmm compare https://github.com/owner/repo
    Run standard benchmark suite

  $ fmm compare https://github.com/owner/repo --quick
    Quick mode with fewer tasks

  $ fmm compare https://github.com/owner/repo --format json -o results/
    JSON output to a specific directory

###### **Arguments:**

* `<URL>` — GitHub repository URL (e.g., https://github.com/owner/repo)

###### **Options:**

* `-b`, `--branch <BRANCH>` — Branch to compare (default: main)
* `--src-path <SRC_PATH>` — Path within repo to analyze
* `--tasks <TASKS>` — Task set to use (standard, quick, or path to custom JSON)

  Default value: `standard`
* `--runs <RUNS>` — Number of runs per task

  Default value: `1`
* `-o`, `--output <OUTPUT>` — Output directory for results
* `--format <FORMAT>` — Output format

  Default value: `both`

  Possible values: `json`, `markdown`, `both`

* `--max-budget <MAX_BUDGET>` — Maximum budget in USD

  Default value: `10.0`
* `--no-cache` — Skip cache (always re-run tasks)
* `--quick` — Quick mode (fewer tasks, faster results)
* `--model <MODEL>` — Model to use

  Default value: `sonnet`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
