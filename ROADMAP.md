# fmm — Roadmap to 10/10

**Created:** 2026-02-13  
**Current Rating:** 8/10  
**Target Rating:** 10/10

---

## Current State

| Category | Score | Notes |
|----------|-------|-------|
| Core Technology | 9/10 | Solid architecture, validated claims |
| Implementation | 7/10 | Works, but no incremental updates |
| Documentation | 7/10 | Good, lacks quick start |
| Testing | 5/10 | Unit tests only, no E2E |
| Integration | 7/10 | MCP works, no IDE/CI |

---

## Target State

| Category | Score | Key Deliverables |
|----------|-------|------------------|
| Core Technology | 10/10 | Semantic search, more languages |
| Implementation | 10/10 | Incremental updates, watch mode |
| Documentation | 10/10 | 60s quick start, diagrams |
| Testing | 10/10 | Integration tests, E2E coverage |
| Integration | 10/10 | VSCode, CI, pre-commit hooks |

---

## Phase 1: Foundation (P0)

### 1.1 Integration Tests

**Problem:** No tests verify the full pipeline works.

**Solution:** Add integration test suite.

```
tests/
├── integration/
│   ├── mod.rs
│   ├── test_skill_loading.rs      # Skill installs correctly
│   ├── test_mcp_server.rs         # MCP tools return correct data
│   ├── test_generate_roundtrip.rs # Generate → validate cycle
│   └── test_cli_commands.rs       # All CLI commands work
├── fixtures/
│   └── sample_project/
│       ├── src/
│       │   ├── index.ts
│       │   ├── utils.ts
│       │   └── store.ts
│       └── expected/
│           ├── index.ts.fmm
│           ├── utils.ts.fmm
│           └── store.ts.fmm
└── e2e/
    └── test_fmm_run.rs            # fmm run produces correct answer
```

**Example test:**
```rust
#[test]
fn test_mcp_lookup_export_returns_correct_file() {
    let temp = TempDir::new().unwrap();
    create_sample_project(temp.path());
    
    // Generate sidecars
    let status = Command::new("fmm")
        .arg("generate")
        .current_dir(temp.path())
        .status()
        .unwrap();
    assert!(status.success());
    
    // Query via MCP
    let mut server = McpServer::new();
    server.load_from_sidecars(temp.path());
    
    let result = server.lookup_export("createStore").unwrap();
    assert_eq!(result.file, "src/store.ts");
    assert!(result.exports.contains(&"createStore".to_string()));
}

#[test]
fn test_skill_installs_to_correct_path() {
    let temp = TempDir::new().unwrap();
    
    let status = Command::new("fmm")
        .args(["init", "--skill"])
        .current_dir(temp.path())
        .status()
        .unwrap();
    assert!(status.success());
    
    let skill_path = temp.path().join(".claude/skills/fmm-navigate/SKILL.md");
    assert!(skill_path.exists(), "Skill should be at {:?}", skill_path);
    
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("mcp__fmm__fmm_lookup_export"));
}
```

**Effort:** 2 days  
**Impact:** High - proves claims work

---

### 1.2 Quick Start Guide

**Problem:** No clear path from zero to value.

**Solution:** Add 60-second quick start to README.

```markdown
## Quick Start

### 1. Install

```bash
cargo install fmm
```

### 2. Generate

```bash
cd your-project
fmm generate
```

### 3. Query

```bash
# Structured query
fmm search --export createStore

# Natural language
fmm run "What's the architecture of this codebase?"
```

**Done.** Your codebase is now navigable by AI with 99% fewer tokens.
```

**Effort:** 2 hours  
**Impact:** High - first impression

---

### 1.3 Architecture Diagram

**Problem:** How it works isn't immediately clear.

**Solution:** Add visual diagram.

```markdown
## How It Works

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
│  Source Files   │────▶│  fmm generate│────▶│  .fmm files │
│  foo.ts         │     │  (tree-sitter)│    │  foo.ts.fmm │
│  bar.py         │     │              │     │  bar.py.fmm │
└─────────────────┘     └──────────────┘     └─────────────┘
   500 files                   │                    │
   ~50,000 lines               │                    │
                               │                    ▼
                         ┌──────────────┐     ┌─────────────┐
                         │  MCP Server  │◀────│  Manifest   │
                         │  (O(1) index) │     │  (in-memory)│
                         └──────────────┘     └─────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
             ┌──────────────┐       ┌──────────────┐
             │  Claude CLI  │       │   fmm run    │
             │  (with MCP)  │       │  (human use) │
             └──────────────┘       └──────────────┘
                    │                       │
                    ▼                       ▼
             "Where is X?"          "Architecture?"
             ~50 tokens             ~500 tokens
             (vs 50,000)            (vs 5,000)
```

### Token Comparison

| Task | Without FMM | With FMM | Reduction |
|------|-------------|----------|-----------|
| Find symbol definition | 50,000 tokens | 50 tokens | 99.9% |
| Dependency analysis | 100,000 tokens | 200 tokens | 99.8% |
| Architecture overview | 200,000 tokens | 500 tokens | 99.7% |
```

**Effort:** 2 hours  
**Impact:** High - instant understanding

---

## Phase 2: Developer Experience (P1)

### 2.1 VSCode Extension

**Problem:** Developers live in IDEs, not terminals.

**Solution:** VSCode extension for sidecar-aware navigation.

**Features:**
- Hover tooltip shows sidecar info (exports, dependencies, LOC)
- "Go to Definition" uses FMM index
- Command palette: `FMM: Find Export`, `FMM: Show Dependencies`
- Sidebar: dependency tree visualization

**Implementation:**
```
fmm-vscode/
├── src/
│   ├── extension.ts
│   ├── sidecarProvider.ts
│   ├── dependencyView.ts
│   └── mcpClient.ts
├── package.json
└── README.md
```

**Key APIs:**
```typescript
// Show sidecar info on hover
vscode.languages.registerHoverProvider(
  { pattern: '**/*.{ts,js,py,rs,go}' },
  {
    provideHover(document, position) {
      const sidecar = loadSidecar(document.uri);
      const symbol = getSymbolAtPosition(document, position);
      return new Hover(formatSidecarInfo(sidecar, symbol));
    }
  }
);

// Command: Find export
vscode.commands.registerCommand('fmm.findExport', async () => {
  const query = await vscode.window.showInputBox({ prompt: 'Export name' });
  const result = await mcpClient.call('fmm_lookup_export', { name: query });
  vscode.window.showTextDocument(Uri.file(result.file));
});
```

**Effort:** 1 week  
**Impact:** High - meets developers where they are

---

### 2.2 Incremental Updates

**Problem:** `fmm generate` regenerates everything on every run.

**Solution:** Only regenerate changed files.

**Algorithm:**
```rust
pub fn generate_incremental(path: &Path) -> Result<usize> {
    let mut updated = 0;
    
    for source_file in discover_source_files(path)? {
        let sidecar = sidecar_path_for(&source_file);
        
        // Skip if sidecar is newer than source
        if sidecar.exists() {
            let source_mtime = fs::metadata(&source_file)?.modified()?;
            let sidecar_mtime = fs::metadata(&sidecar)?.modified()?;
            if sidecar_mtime > source_mtime {
                continue;
            }
        }
        
        // Generate single sidecar
        generate_sidecar(&source_file)?;
        updated += 1;
    }
    
    Ok(updated)
}
```

**CLI:**
```bash
fmm generate              # Incremental (only changed files)
fmm generate --all        # Force regenerate all
fmm generate --dry-run    # Show what would change
```

**Effort:** 3 days  
**Impact:** Medium - faster for large repos

---

### 2.3 Watch Mode

**Problem:** Developers forget to regenerate sidecars.

**Solution:** Auto-generate on file change.

```bash
fmm watch

# Output:
Watching for changes... (Ctrl+C to stop)
✓ Updated src/auth.ts.fmm
✓ Updated src/store.ts.fmm
```

**Implementation:**
```rust
pub fn watch(path: &Path) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(path, RecursiveMode::Recursive)?;
    
    println!("Watching for changes... (Ctrl+C to stop)");
    
    for res in rx {
        match res {
            Ok(event) => {
                for path in event.paths {
                    if is_source_file(&path) {
                        generate_sidecar(&path)?;
                        println!("✓ Updated {}", sidecar_path_for(&path).display());
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    }
    
    Ok(())
}
```

**Effort:** 2 days  
**Impact:** Medium - always up-to-date

---

## Phase 3: CI/CD Integration (P1)

### 3.1 GitHub Action

**Problem:** Sidecars get stale in CI.

**Solution:** Official GitHub Action.

```yaml
# .github/workflows/fmm.yml
name: FMM Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install FMM
        uses: mdcontext/fmm-action@v1
        
      - name: Validate sidecars
        run: fmm validate --fail-on-stale
        
      - name: Generate if stale
        if: failure()
        run: |
          fmm generate
          echo "Sidecars were stale. Regenerated."
```

**Action definition:**
```yaml
# action.yml
name: 'FMM Setup'
description: 'Install and configure FMM for codebase metadata'
inputs:
  version:
    description: 'FMM version'
    required: false
    default: 'latest'
runs:
  using: 'composite'
  steps:
    - run: cargo install fmm --version ${{ inputs.version }}
      shell: bash
```

**Effort:** 1 day  
**Impact:** Medium - CI hygiene

---

### 3.2 Pre-commit Hook

**Problem:** Stale sidecars get committed.

**Solution:** Auto-generate before commit.

```bash
# .git/hooks/pre-commit
#!/bin/bash

# Get list of staged source files
STAGED=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(ts|js|py|rs|go)$')

if [ -n "$STAGED" ]; then
    echo "Generating sidecars for staged files..."
    echo "$STAGED" | xargs -I{} fmm generate {}
    echo "$STAGED" | sed 's/$/.fmm/' | xargs git add
fi
```

**Setup command:**
```bash
fmm init --pre-commit
# Creates .git/hooks/pre-commit
```

**Effort:** 4 hours  
**Impact:** Low - nice to have

---

## Phase 4: Enhanced Features (P2)

### 4.1 Progress Indicator

**Problem:** Large repos have no feedback during generation.

**Solution:** Progress bar.

```bash
fmm generate

Processing... ████████████████░░░░░░░░ 65% | 845/1300 files | 2.3s
             └─ src/services/
```

**Implementation:**
```rust
use indicatif::{ProgressBar, ProgressStyle};

let pb = ProgressBar::new(files.len() as u64);
pb.set_style(ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
    .progress_chars("#>-"));

for file in files {
    generate_sidecar(&file)?;
    pb.inc(1);
}

pb.finish_with_message("Done");
```

**Effort:** 4 hours  
**Impact:** Low - polish

---

### 4.2 Better Error Messages

**Problem:** Parse errors are unhelpful.

**Solution:** Contextual errors with hints.

**Current:**
```
Error: Failed to parse src/auth.ts
```

**Better:**
```
Error: Failed to parse src/auth.ts:42

  40 |   async function handleAuth() {
  41 |    const user = await fetchUser()
  42 |    if (user
          ^^^^^^^^
  Expected '}' to close function body
  
  Hint: Check for unclosed braces after line 41
```

**Effort:** 2 days  
**Impact:** Low - developer experience

---

### 4.3 Dependency Visualization

**Problem:** Dependency graphs are hard to understand.

**Solution:** Visual output.

```bash
fmm deps --visual src/index.ts

# Output: deps.svg
```

**Graph:**
```
┌─────────────────┐
│   src/index.ts  │
│   (entry point) │
└────────┬────────┘
         │
    ┌────┴────┬──────────┐
    ▼         ▼          ▼
┌───────┐ ┌───────┐ ┌───────────┐
│auth.ts│ │api.ts │ │ store.ts  │
│(3 dep)│ │(5 dep)│ │ (12 dep)  │
└───────┘ └───────┘ └─────┬─────┘
                          │
                    ┌─────┴─────┐
                    ▼           ▼
              ┌──────────┐ ┌──────────┐
              │ utils.ts │ │ types.ts │
              │ (0 dep)  │ │ (0 dep)  │
              └──────────┘ └──────────┘
```

**Effort:** 3 days  
**Impact:** Medium - architecture understanding

---

## Phase 5: Advanced Features (P3)

### 5.1 Semantic Search

**Problem:** Text-based search misses intent.

**Solution:** Embedding-based similarity.

```bash
fmm search --semantic "authentication logic"
# Returns: auth.ts, login.ts, session.ts (ranked by semantic similarity)
```

**Implementation:**
```rust
pub fn semantic_search(query: &str, manifest: &Manifest) -> Vec<SearchResult> {
    let query_embedding = embed(query);
    
    let mut results: Vec<_> = manifest.files
        .iter()
        .map(|(file, entry)| {
            let file_embedding = get_cached_embedding(file);
            let similarity = cosine_similarity(&query_embedding, &file_embedding);
            (file, entry, similarity)
        })
        .collect();
    
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    results
}
```

**Effort:** 1 week  
**Impact:** Medium - better discovery

---

### 5.2 Additional Languages

**Problem:** Only 9 languages supported.

**Solution:** Add more language parsers.

**Priority order:**
1. Kotlin (Android teams)
2. Swift (iOS teams)
3. Scala (enterprise)
4. PHP (WordPress/Drupal)
5. Elixir (BEAM ecosystem)

**Per-language effort:** 1-2 days  
**Impact:** Low per language, high cumulative

---

### 5.3 Cross-Repository Tracking

**Problem:** Modern codebases span multiple repos.

**Solution:** Link repos and track cross-repo dependencies.

```bash
# Link a shared library
fmm link ../shared-components

# Now queries include cross-repo results
fmm search --export Button
# Returns: ../shared-components/src/Button.tsx
```

**Effort:** 1 week  
**Impact:** Medium - monorepo/multi-repo support

---

## Summary

### Effort vs Impact Matrix

| Feature | Effort | Impact | Priority |
|---------|--------|--------|----------|
| Integration tests | 2 days | High | P0 |
| Quick start guide | 2 hours | High | P0 |
| Architecture diagram | 2 hours | High | P0 |
| VSCode extension | 1 week | High | P1 |
| Incremental updates | 3 days | Medium | P1 |
| Watch mode | 2 days | Medium | P1 |
| GitHub Action | 1 day | Medium | P1 |
| Pre-commit hook | 4 hours | Low | P2 |
| Progress indicator | 4 hours | Low | P2 |
| Better errors | 2 days | Low | P2 |
| Dependency viz | 3 days | Medium | P2 |
| Semantic search | 1 week | Medium | P3 |
| More languages | 2 days/ea | Low | P3 |
| Cross-repo | 1 week | Medium | P3 |

### Timeline

```
Week 1-2:  P0 (Foundation)
           - Integration tests
           - Quick start
           - Architecture diagram

Week 3-4:  P1 (Developer Experience)
           - VSCode extension
           - Incremental updates
           - Watch mode
           - GitHub Action

Week 5-6:  P2 (Polish)
           - Progress indicator
           - Better errors
           - Dependency visualization

Week 7+:   P3 (Advanced)
           - Semantic search
           - More languages
           - Cross-repo tracking
```

### Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Test coverage | ~60% | 95% |
| Time to first value | 10 min | 60 sec |
| Supported languages | 9 | 15 |
| IDE support | None | VSCode, Vim |
| CI integration | Manual | 1-click |

---

*Document created: 2026-02-13*
