# FMM Data Formats: Complete Documentation

## Overview

FMM (Frontmatter Matters) generates structured metadata about source code in two forms:

1. **Sidecar files** (`.fmm`): YAML files adjacent to source files
2. **Manifest file** (`.fmm/index.json`): Aggregated JSON index queryable by LLMs

The critical insight: **Sidecars are the source of truth. Manifest aggregates sidecars into a queryable index.**

---

## 1. Sidecar File Format (.fmm)

### Location and Naming

- **Pattern**: `<source_file>.fmm`
- **Examples**:
  - `src/auth.ts` → `src/auth.ts.fmm`
  - `lib/utils.py` → `lib/utils.py.fmm`
  - `cli/mod.rs` → `cli/mod.rs.fmm`

### YAML Structure

Sidecars are plain YAML files with the following fields:

```yaml
file: src/auth/session.ts
fmm: v0.2
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis]
dependencies: [./types, ./config]
loc: 234
modified: 2026-01-30
```

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | String | ✓ | Relative path to source file (normalized path) |
| `fmm` | String | ✗ | Format version (e.g., "v0.2"). Optional; if present, indicates sidecar version |
| `exports` | Array[String] | ✓ | Names of public symbols: functions, classes, types, constants, variables |
| `imports` | Array[String] | ✓ | External package/module imports (excludes relative/local imports) |
| `dependencies` | Array[String] | ✓ | Local relative imports (e.g., "./types", "../config", "crate::utils") |
| `loc` | Integer | ✓ | Lines of code (non-blank, non-comment lines) |
| `modified` | String | ✓ | Last modified date (ISO format: YYYY-MM-DD) |
| `<language>` | Object | ✗ | Language-specific custom fields (nested section, see below) |

### Language-Specific Custom Fields

For each language, additional language-specific metadata can be included:

#### TypeScript/JavaScript

```yaml
file: src/auth.ts
exports: [createSession]
...
typescript:
  decorators: [memoize, deprecated]
```

#### Python

```yaml
file: src/processor.py
exports: [DataProcessor]
...
python:
  decorators: [property, staticmethod, classmethod]
  dunder_all: [fetch_data, transform, DataProcessor]
```

#### Rust

```yaml
file: src/lib.rs
exports: [Config, Pipeline, process]
...
rust:
  async_functions: 2
  derives: [Clone, Debug, Deserialize, Serialize]
  lifetimes: ['a, 'static]
  trait_impls: [Display for Error, Iterator for Pipeline]
  unsafe_blocks: 1
```

#### Go

```yaml
file: main.go
exports: [Handler, Process]
...
go:
  interfaces: [io.Writer, fmt.Stringer]
  goroutines: 3
```

#### Java

```yaml
file: src/AuthService.java
exports: [AuthService, authenticate]
...
java:
  annotations: [Override, Deprecated, FunctionalInterface]
  visibility: public
```

#### C#

```yaml
file: Auth.cs
exports: [AuthManager, ValidateToken]
...
csharp:
  attributes: [Obsolete, Serializable]
  async_methods: 1
  properties: [Token, User]
```

#### C++

```yaml
file: auth.cpp
exports: [authenticate, Session]
...
cpp:
  templates: [AuthHandler<T>, Result<T>]
  namespaces: [auth, security]
```

#### Ruby

```yaml
file: processor.rb
exports: [DataProcessor, process_data]
...
ruby:
  attr_accessor: [config, status]
  attr_reader: [result]
  metaprogramming: ["define_method", "class_eval"]
```

### YAML List Format

Arrays in sidecars use inline YAML syntax:

```yaml
exports: [func1, func2, ClassA, ClassB]
imports: [lodash, react, axios]
dependencies: [./types, ../config, ./utils/helpers]
```

Empty lists are represented as:

```yaml
imports: []
```

### Complete Real Example

```yaml
file: src/api/controllers/authController.ts
fmm: v0.2
exports: [AuthController, createAuthController]
imports: []
dependencies: [../../auth/login, ../../auth/signup, ../../auth/types, ../../services/audit]
loc: 31
modified: 2026-01-29
typescript:
  decorators: [Injectable, Controller]
```

---

## 2. Manifest Format (.fmm/index.json)

### Location

- **Standard location**: `.fmm/index.json` in the project root
- **Generated automatically** by fmm tooling
- **Version 1.0** (see structure below for version 2.0)

### JSON Schema

```json
{
  "version": "1.0",
  "generated": "2026-01-28T06:20:52.917491Z",
  "files": {
    "<file_path>": {
      "exports": [/* array of strings */],
      "imports": [/* array of strings */],
      "dependencies": [/* array of strings */],
      "loc": <number>
    }
  },
  "exportIndex": {
    "<export_name>": "<file_path>",
    ...
  }
}
```

### Field Reference

#### Root Level

| Field | Type | Description |
|-------|------|-------------|
| `version` | String | Schema version. Currently "1.0". Will upgrade to "2.0" with language-specific fields. |
| `generated` | String | ISO 8601 timestamp when manifest was generated |
| `files` | Object | Map of file paths → metadata (see below) |
| `exportIndex` | Object | Reverse index: export name → file path (for rapid lookups) |

#### File Entry Object

```json
{
  "exports": ["createSession", "validateSession"],
  "imports": ["jwt", "redis"],
  "dependencies": ["./types", "./config"],
  "loc": 234
}
```

| Field | Type | Description |
|-------|------|-------------|
| `exports` | Array[String] | Public symbols exported by this file |
| `imports` | Array[String] | External package dependencies |
| `dependencies` | Array[String] | Local relative imports |
| `loc` | Integer | Lines of code |

#### Export Index Object

**Purpose**: Fast lookup of where an export is defined.

```json
"exportIndex": {
  "createSession": "api/models/session.ts",
  "SessionManager": "src/auth/session.ts",
  "validateToken": "src/auth/jwt.ts",
  ...
}
```

**Behavior with TS/JS Priority**: If both `foo.ts` and `foo.js` export `myFunc`, the manifest prioritizes `.ts` over `.js`:

```json
{
  "files": {
    "src/lib.ts": { "exports": ["myFunc"], ... },
    "src/lib.js": { "exports": ["myFunc"], ... }
  },
  "exportIndex": {
    "myFunc": "src/lib.ts"  // TypeScript takes priority
  }
}
```

### Complete Real Example

```json
{
  "version": "1.0",
  "generated": "2026-01-29T11:53:20.336520Z",
  "files": {
    "utils/id.ts": {
      "exports": ["extractTimestamp", "generateId", "isValidId"],
      "imports": [],
      "dependencies": [],
      "loc": 18
    },
    "auth/jwt.ts": {
      "exports": ["generateToken", "refreshToken", "verifyToken"],
      "imports": [],
      "dependencies": ["../auth/types", "../config/app"],
      "loc": 36
    },
    "api/routes/auth.ts": {
      "exports": ["getAuthRoutes"],
      "imports": [],
      "dependencies": [
        "../../auth/jwt",
        "../../auth/login",
        "../../auth/signup",
        "../../middleware/auth",
        "../../middleware/rateLimit"
      ],
      "loc": 84
    }
  },
  "exportIndex": {
    "extractTimestamp": "utils/id.ts",
    "generateId": "utils/id.ts",
    "isValidId": "utils/id.ts",
    "generateToken": "auth/jwt.ts",
    "refreshToken": "auth/jwt.ts",
    "verifyToken": "auth/jwt.ts",
    "getAuthRoutes": "api/routes/auth.ts"
  }
}
```

---

## 3. Relationship: Sidecars ↔ Manifest

### Data Flow

```
Source Code (TypeScript, Python, Rust, etc.)
    ↓
Parser (tree-sitter based extraction)
    ↓
Metadata { exports, imports, dependencies, loc, custom_fields }
    ↓
Frontmatter Formatter
    ↓
Sidecar YAML (.fmm file) [SOURCE OF TRUTH]
    ↓
Manifest Generator (reads all .fmm files)
    ↓
Manifest JSON (.fmm/index.json) [QUERYABLE INDEX]
```

### Key Points

1. **Sidecars are source of truth**
   - Generated by parsers from actual source code
   - Stored alongside source files
   - Can be version controlled

2. **Manifest is derived**
   - Built by reading and aggregating all `.fmm` sidecar files
   - Provides fast lookup via exportIndex
   - Optimized for LLM consumption (single JSON file)

3. **Generation Process**
   ```rust
   // From manifest/mod.rs
   pub fn load_from_sidecars(root: &Path) -> Result<Self> {
       // 1. Walk root directory
       // 2. Find all *.fmm files
       // 3. Parse each sidecar YAML
       // 4. Build export_index reverse map
       // 5. Return aggregated Manifest
   }
   ```

4. **Update Pattern**
   - When source changes: regenerate sidecar
   - When sidecar changes: regenerate manifest
   - Typically automated via git hooks or CI

---

## 4. Language-Specific Extraction Details

### TypeScript/JavaScript

**Exports detected**:
- `export function foo() {}`
- `export const bar = ...`
- `export class Baz {}`
- `export interface IFoo {}`
- `export { x, y } from "./lib"`

**Imports detected**:
- `import x from "package"` → "package"
- Excludes relative imports (starting with `.` or `/`)

**Dependencies detected**:
- `import x from "./local"` → "./local"
- `import x from "../parent"` → "../parent"

**Custom fields (TypeScript)**:
- `decorators`: Detected `@Decorator` usage

### Python

**Exports detected** (in priority order):
- `__all__` list if present (most authoritative)
- Top-level function definitions
- Top-level class definitions
- Top-level constants (UPPERCASE)
- Excludes names starting with `_`

**Imports detected**:
- `import package` → "package"
- `import package.submodule` → "package"
- Excludes relative imports (starting with `.`)

**Dependencies detected**:
- `from . import x` → "."
- `from .. import x` → ".."
- `from .utils import x` → ".utils"

**Custom fields (Python)**:
- `decorators`: List of decorator names
- `dunder_all`: Values from `__all__`

### Rust

**Exports detected**:
- Items with `pub` visibility modifier:
  - `pub fn name()`
  - `pub struct Name`
  - `pub enum Name`
  - `pub trait Name`
  - `pub type Name`
  - `pub const NAME`
  - `pub mod name`

**Imports detected**:
- `use std::path::Path` → "std"
- `use tokio` → "tokio"
- `use serde::{Serialize, Deserialize}` → "serde"
- Excludes crate-local imports

**Dependencies detected**:
- `use crate::utils` → "crate"
- `use super::config` → "super"
- `use self::types` → "self"

**Custom fields (Rust)**:
- `async_functions`: Count
- `derives`: List from `#[derive(...)]`
- `lifetimes`: Named lifetime parameters
- `trait_impls`: Trait implementations
- `unsafe_blocks`: Count

### Python | Go | Java | C# | C++ | Ruby

Detailed extraction logic follows similar patterns:
- External vs. local import classification
- Visibility modifier detection
- Language-specific AST traversal
- Custom metadata collection

See `src/parser/builtin/` for implementation details.

---

## 5. How LLMs Consume These Formats

### Query Pattern 1: Codebase Overview (Recommended)

**LLM receives manifest in system prompt:**

```
You have access to a codebase manifest. Query it to understand the structure:

{
  "version": "1.0",
  "files": { ... },
  "exportIndex": { ... }
}

User query: "How is authentication implemented?"
```

**LLM process**:
1. Search exportIndex for "auth"-related exports
2. Find files: `auth/login.ts`, `auth/jwt.ts`, `middleware/auth.ts`
3. Request specific files from user only if detailed inspection needed
4. Generates response based on manifest + context

**Token cost**: ~1,500 tokens for manifest vs. ~50,000 tokens reading all files

### Query Pattern 2: "Where is X defined?"

**LLM receives**:
```
Find where validateToken is exported:

exportIndex: {
  "validateToken": "src/auth/jwt.ts",
  ...
}
```

**Result**: Instant answer without reading files

### Query Pattern 3: Dependency Analysis

**LLM receives**:
```
files: {
  "api/routes/auth.ts": {
    "dependencies": ["../../auth/jwt", "../../auth/login"]
  }
}
```

**LLM queries**: "Which files does auth route depend on?" → Direct answer

### Query Pattern 4: Export Chain

**Manifest query workflow**:
```
1. LLM searches exportIndex for "createSession"
   → Found in "api/models/session.ts"
2. LLM reads entry for that file
   → dependencies: ["../../utils/id"]
3. LLM searches exportIndex for files in "utils/id"
   → Chains through dependency graph
```

---

## 6. Evolution: From Inline Frontmatter to Sidecars

### Phase 1: Inline Frontmatter (Abandoned)

```typescript
// --- FMM ---
// file: ./auth.ts
// exports: [validateUser, createSession]
// imports: [crypto]
// loc: 234
// ---
```

**Problem**: LLMs skip comment blocks as "noise" → frontmatter invisible

### Phase 2: Code-Level Metadata (Considered)

```typescript
export const __meta = {
  exports: ["validateUser", "createSession"],
  imports: ["crypto"],
  loc: 234
};
```

**Problem**: Pollutes source files, confuses bundlers

### Phase 3: Tool-Level Extraction (Inefficient)

LLM tools parse special markers and extract metadata separately

**Problem**: Requires tool modifications, vendor dependent

### Phase 4: Manifest JSON File (Current Solution)

```
.fmm/
  index.json  ← Single queryable file
```

**Advantages**:
- No source file changes required
- JSON is natively parseable by LLMs
- Single file query = entire codebase understanding
- Generated automatically via static analysis
- Synced via git hooks / CI / watch mode
- 94%+ token reduction for LLM consumption

**Key insight**: Frontmatter in comments = human-readable but invisible to LLMs. Manifest JSON = LLM-optimized infrastructure.

---

## 7. Supported Languages

| Language | Extension | Custom Fields | Notes |
|----------|-----------|---------------|-------|
| TypeScript | .ts, .tsx | decorators | JSX support included |
| JavaScript | .js, .jsx | decorators | JSX support included |
| Python | .py | decorators, dunder_all | Respects `__all__` |
| Rust | .rs | async, derives, lifetimes, traits, unsafe | Full feature extraction |
| Go | .go | interfaces, goroutines | Interface tracking |
| Java | .java | annotations, visibility | Full OOP support |
| C# | .cs | attributes, properties, async | .NET features |
| C++ | .cpp, .hpp, etc. | templates, namespaces | Modern C++ |
| Ruby | .rb | attr_*, metaprogramming | Dynamic feature tracking |

---

## 8. Configuration (.fmmrc.json)

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024
}
```

| Option | Type | Default | Purpose |
|--------|------|---------|---------|
| `languages` | Array | ["ts", "js", "py", "rs", "go"] | Which file types to process |
| `format` | String | "yaml" | Sidecar format (currently YAML only) |
| `include_loc` | Boolean | true | Include line count |
| `include_complexity` | Boolean | false | Include complexity metrics (reserved) |
| `max_file_size` | Number | 1024 | Max file size in KB to process |

---

## 9. Practical Workflow

### For Developers

```bash
# Generate sidecars for new files
fmm generate src/

# Update sidecars when files change
fmm update src/

# Validate sidecars match source
fmm validate src/

# Generate manifest
fmm manifest --output .fmm/index.json
```

### For LLMs (Claude Agent Pattern)

```
1. Read .fmm/index.json (1,500 tokens) ← ONE QUERY
2. Understand entire codebase structure
3. Targeted file reads as needed
4. Query export index for symbol location
```

---

## 10. File Size and Performance

### Typical Manifest Sizes

| Codebase | Files | Manifest Size |
|----------|-------|---------------|
| Small (10 files) | 10 | ~3 KB |
| Medium (100 files) | 100 | ~25 KB |
| Large (1000 files) | 1000 | ~250 KB |

**Result**: Manifest fits comfortably in LLM context window, leaving room for actual code

---

## Summary

| Aspect | Sidecar (.fmm) | Manifest (index.json) |
|--------|---|---|
| **Format** | YAML | JSON |
| **Location** | Alongside source files | `.fmm/index.json` |
| **Source of truth** | Yes | No (derived) |
| **Purpose** | Per-file metadata | Aggregated index |
| **LLM consumption** | Rarely (unless reading file) | Primary (single query) |
| **Human-readable** | Yes | Yes |
| **Update trigger** | Source file changes | Sidecar files change |
| **Searchability** | Via grep | Direct JSON query |
| **Size** | ~50 bytes average | ~25 KB per 100 files |

**The core value**: LLMs query ONE manifest file instead of reading 100+ source files = 94-97% token savings = 94-97% cost reduction.
