# Does Frontmatter Matter?

This is the only question. Everything else is noise.

## The Experiment

Two runs. Same issue. Same model. Same budget. One has fmm, one doesn't.

### Control (no fmm)
1. Clone the repo
2. Point Claude at the issue
3. Measure: tokens consumed, files read, outcome

### Treatment (with fmm)
1. Clone the repo
2. `fmm generate`
3. Point Claude at the issue with sidecar-resolved context
4. Measure: tokens consumed, files read, outcome

### What We Measure

| Metric | How |
|--------|-----|
| **Input tokens** | Total tokens Claude consumed to reach a solution |
| **Files read** | Number of source files Claude opened |
| **Outcome** | Did it produce a plausible fix? (human judgment, binary) |
| **Cost** | USD spent per run |
| **Wall time** | Seconds from start to commit |

### What Constitutes Proof

fmm matters if, across N issues:
- Treatment uses **fewer tokens** than control (efficiency)
- Treatment reads **fewer files** than control (navigation)
- Treatment produces **equal or better fixes** (quality)

If treatment uses fewer tokens but produces worse fixes, fmm is a cost optimization, not a quality improvement. Both are interesting but the README claims navigation quality.

## Running the Experiment

```bash
# Single issue, shows both runs side by side
fmm gh issue https://github.com/owner/repo/issues/123 --compare

# Batch: run a corpus of issues
fmm gh batch proofs/corpus.json --compare
```

### Corpus Format

`proofs/corpus.json`:
```json
[
  {
    "url": "https://github.com/openclaw/openclaw/issues/5492",
    "tags": ["bug", "python", "medium"]
  },
  {
    "url": "https://github.com/ruvnet/claude-flow/issues/1047",
    "tags": ["bug", "typescript", "small"]
  }
]
```

Tags are for analysis (group results by language, complexity). The experiment doesn't use them — they're metadata for the human reviewing results.

### Adding Your Own Issues

Anyone can reproduce or extend this experiment:

1. Fork this repo
2. Add issues to `proofs/corpus.json`
3. Run `fmm gh batch proofs/corpus.json --compare`
4. Results land in `proofs/results/`

### What Makes a Good Test Issue

- Has file paths or symbol names in the body (so fmm has something to resolve)
- Is a real bug or feature request (not a question or discussion)
- The repo is public and cloneable
- The issue is solvable by reading code and making changes

### What Makes a Bad Test Issue

- "App is slow" (no code references, no clear fix)
- Issues in private repos
- Issues that require running the app to reproduce

## Results

Results are committed to `proofs/results/` as they're generated. Each run produces:

```
proofs/results/
  <issue-id>/
    control.json    # Token usage, files read, outcome
    treatment.json  # Token usage, files read, outcome
    comparison.md   # Side-by-side summary
```

## Current Status

**Not yet run.** Experiment infrastructure exists (`fmm gh issue --compare`). Corpus needs to be built and first runs executed.

## Why This Matters

If fmm doesn't produce measurably better outcomes, it's a solution looking for a problem. The README claims 88-97% token reduction — that's an efficiency claim. This experiment tests the deeper claim: does structured metadata make AI agents navigate code better?
