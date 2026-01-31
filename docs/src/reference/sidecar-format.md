# Sidecar Format

fmm sidecar files use YAML frontmatter format. Each `.fmm` file describes the metadata of its companion source file.

## File naming

Sidecars are placed alongside their source files with an `.fmm` extension appended:

```
src/auth.ts      → src/auth.ts.fmm
src/db.py        → src/db.py.fmm
src/lib.rs       → src/lib.rs.fmm
```

## Schema

```yaml
---
file: <relative-path>
exports: [<symbol>, ...]
imports: [<package>, ...]
dependencies: [<relative-path>, ...]
loc: <integer>
---
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `file` | string | Relative path from project root to the source file |
| `exports` | string[] | Public symbols: functions, classes, types, constants, interfaces |
| `imports` | string[] | External package imports (npm packages, pip packages, crate names) |
| `dependencies` | string[] | Relative file paths this file depends on (internal imports) |
| `loc` | integer | Total lines of code in the source file |

### Language-specific custom fields

Some languages include additional metadata:

**Rust:**
```yaml
rust:
  derives: [Debug, Clone, Serialize]
  async_functions: [fetch_data, process_stream]
  unsafe_blocks: 2
  lifetimes: ['a, 'static]
  trait_impls: [Display for Config, From<String> for Error]
```

**Python:**
```yaml
python:
  decorators: [app.route, dataclass, abstractmethod]
```

## Example

For a TypeScript file `src/store/index.ts`:

```typescript
import { createSlice, configureStore } from '@reduxjs/toolkit';
import { api } from '../api/client';

export interface StoreConfig {
  debug: boolean;
}

export function createStore(config: StoreConfig) {
  return configureStore({ /* ... */ });
}

export const defaultConfig: StoreConfig = { debug: false };
```

The generated sidecar `src/store/index.ts.fmm`:

```yaml
---
file: src/store/index.ts
exports: [StoreConfig, createStore, defaultConfig]
imports: ["@reduxjs/toolkit"]
dependencies: [../api/client]
loc: 14
---
```

## What's excluded

- **Private symbols** — unexported functions, internal helpers
- **Relative imports** — captured as `dependencies`, not `imports`
- **Type-only imports** — included in `imports` (they affect the dependency graph)
- **Comments and whitespace** — not tracked, but `loc` counts all lines

## Manifest (in-memory)

When fmm loads sidecars, it builds an in-memory manifest with:
- **File index** — O(1) lookup by file path
- **Export reverse index** — O(1) lookup: "which file exports `createStore`?"
- **Full metadata** — all sidecar fields accessible per file
