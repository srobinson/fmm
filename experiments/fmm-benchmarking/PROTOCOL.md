# FMM Benchmarking Protocol

## Objective

Measure the difference in file reads and accuracy when Claude navigates a codebase:
- **Control:** No fmm manifest, no instructions
- **FMM:** With manifest + CLAUDE.md instructions

## Test Repository

**zustand** (pmndrs/zustand)
- 16 source files
- State management library
- Clear export structure

## Test Tasks

Run these 5 tasks in each variant. Record for each:
- Number of Read tool calls
- Files accessed (list them)
- Correct answer? (Y/N)
- Time to answer

### Task 1: Find Export
**Prompt:** "Which file exports createStore?"
**Expected:** src/vanilla.ts

### Task 2: Reverse Dependency
**Prompt:** "What files depend on (import from) ./vanilla.ts?"
**Expected:** src/react.ts, src/traditional.ts, src/middleware/*.ts (6-7 files)

### Task 3: Find Implementation
**Prompt:** "Find the persist middleware implementation and tell me what it exports"
**Expected:** src/middleware/persist.ts exports: PersistOptions, PersistStorage, StateStorage, createJSONStorage, persist

### Task 4: Trace Chain
**Prompt:** "The redux middleware - what other middleware does it depend on?"
**Expected:** src/middleware/redux.ts depends on ./devtools.ts

### Task 5: Architecture Overview
**Prompt:** "Give me a quick overview of the module structure - what are the main entry points?"
**Expected:** src/index.ts, src/react.ts, src/vanilla.ts, src/middleware.ts

## Measurement

### Control Run
1. Start fresh Claude session in experiments/fmm-benchmarking/control/
2. No CLAUDE.md modifications
3. Run each task, record metrics

### FMM Run
1. Start fresh Claude session in experiments/fmm-benchmarking/fmm/
2. Add FMM instructions to ~/.claude/CLAUDE.md (see CLAUDE-FMM.md)
3. Run each task, record metrics

## Results Template

| Task | Control Reads | FMM Reads | Control Correct | FMM Correct |
|------|---------------|-----------|-----------------|-------------|
| 1    |               |           |                 |             |
| 2    |               |           |                 |             |
| 3    |               |           |                 |             |
| 4    |               |           |                 |             |
| 5    |               |           |                 |             |
| Total|               |           |                 |             |
