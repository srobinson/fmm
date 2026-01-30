# FMM Data Formats - Complete Schema Reference

## Overview

fmm produces two data formats that work together: **sidecar files** (`.fmm`) containing per-file metadata in a YAML-like syntax, and an **in-memory manifest** that aggregates all sidecars into a queryable index with O(1) export lookup. This document specifies both formats completely, with real examples derived from the codebase fixtures.

The critical design insight: sidecars are the source of truth, co-located with source files for git-friendliness. The manifest is derived, ephemeral, and built on-demand.

---

## 1. Sidecar Files (.fmm)

### 1.1 File Naming Convention

Every source file gets a companion sidecar by appending `.fmm` to the full filename:

```
src/auth.ts        -> src/auth.ts.fmm
src/lib.rs         -> src/lib.rs.fmm
app.py             -> app.py.fmm
pkg/server.go      -> pkg/server.go.fmm
```

The function `sidecar_path_for()` in `src/extractor/mod.rs` implements this:

```rust
pub fn sidecar_path_for(path: &Path) -> PathBuf {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(".fmm");
    PathBuf::from(sidecar)
}
```

Sidecars live adjacent to their source files, not in a centralized directory. This co-location is a deliberate design decision for git-friendliness and composability (see Architecture doc for full rationale).

### 1.2 Format Specification

Sidecars use a **YAML-like** syntax that is intentionally simpler than strict YAML. The format is rendered by `src/formatter/mod.rs` and parsed by `src/manifest/mod.rs` using line-by-line prefix matching -- no YAML library required.

#### Field Order (Strict)

Fields are always rendered in this exact order:

```yaml
file: <relative-path>
fmm: <version>
exports: [<symbol>, ...]
imports: [<package>, ...]
dependencies: [<local-path>, ...]
loc: <number>
modified: <YYYY-MM-DD>
<language>:
  <custom-field>: <value>
```

The `file:` field is always first. This is intentional: when an LLM reads a sidecar, the very first line tells it which source file this metadata describes.

#### Field Reference

| Field | Type | Presence | Description |
|-------|------|----------|-------------|
| `file` | string | Always | Relative path from project root to source file |
| `fmm` | string | Always (v0.2) | Format version. Currently `v0.2` |
| `exports` | inline array | Conditional | Public/exported symbols. Omitted if empty |
| `imports` | inline array | Conditional | External package imports. Omitted if empty |
| `dependencies` | inline array | Conditional | Local relative imports. Omitted if empty |
| `loc` | integer | Always | Total lines of code |
| `modified` | date string | Always | Generation date in ISO `YYYY-MM-DD` format |
| `<language>:` | nested section | Conditional | Language-specific fields. Omitted if no custom data |

#### Array Syntax

Arrays use inline bracket notation, never block YAML:

```yaml
exports: [createSession, validateSession, destroySession]
imports: [jwt, redis-client]
dependencies: [./types, ./config]
```

Empty arrays cause the entire field to be omitted (not rendered as `exports: []`). This is deliberate -- fewer lines means fewer tokens for LLM consumption.

The rendering logic in `formatter/mod.rs`:

```rust
if !self.metadata.exports.is_empty() {
    lines.push(format!("exports: [{}]", self.metadata.exports.join(", ")));
}
```

#### Parsing Algorithm

The manifest parser (`manifest/mod.rs::parse_sidecar`) uses simple prefix stripping -- no regex, no YAML library:

```rust
for line in content.lines() {
    let line = line.trim();
    if let Some(val) = line.strip_prefix("file: ") {
        file_path = val.to_string();
    } else if let Some(val) = line.strip_prefix("exports: ") {
        exports = parse_yaml_list(val);
    } else if let Some(val) = line.strip_prefix("imports: ") {
        imports = parse_yaml_list(val);
    } else if let Some(val) = line.strip_prefix("dependencies: ") {
        dependencies = parse_yaml_list(val);
    } else if let Some(val) = line.strip_prefix("loc: ") {
        loc = val.parse().unwrap_or(0);
    }
}
```

Inline arrays are parsed by `parse_yaml_list`:

```rust
fn parse_yaml_list(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        if inner.is_empty() {
            return Vec::new();
        }
        inner.split(',').map(|item| item.trim().to_string()).collect()
    } else {
        Vec::new()
    }
}
```

A sidecar is considered invalid (and silently skipped) if the `file:` field is empty or missing.

### 1.3 Core Metadata Fields

#### exports

Public symbols extracted by language-specific AST queries. What counts as an "export" varies by language:

| Language | Export Detection Rule |
|----------|---------------------|
| TypeScript/JS | `export function`, `export class`, `export interface`, `export const/let/var`, `export { ... }`, `export default` |
| Python | Top-level `def`, `class`, module-level assignments. Filtered by `__all__` if present. Names starting with `_` excluded |
| Rust | `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub type`, `pub const`, `pub static`, `pub mod`. Includes `pub(crate)` and `pub(super)` |
| Go | Capitalized identifiers: functions, types, consts, vars (Go convention) |
| Java | `public class`, `public interface`, `public enum` declarations |
| C++ | Classes, structs, enums, functions in namespace scope |
| C# | `public class`, `public interface`, `public enum` declarations. `internal` excluded |
| Ruby | Top-level `class`, `module`, `def` at module scope. `_`-prefixed excluded |

#### imports

External package dependencies. These are imports from third-party or standard library packages, **not** relative/local imports:

- **TypeScript/JS:** `import ... from 'package'` where the specifier does not start with `.` or `/`
- **Python:** `import package`, `from package import ...` where package is not relative
- **Rust:** `use crate_name::...` (filters out `std`, `core`)
- **Go:** Standard library packages in the import block
- **Java:** `import package.Class` statements
- **C#:** `using Namespace` declarations
- **Ruby:** `require 'gem_name'` (not `require_relative`)

#### dependencies

Local relative imports within the project -- the internal module connections:

- **TypeScript/JS:** Import specifiers starting with `.` or `/` (e.g., `./types`, `../utils`)
- **Python:** Relative imports (e.g., `from .utils import`, `from ..models import`)
- **Rust:** `crate::`, `super::` references
- **Go:** External module paths (e.g., `github.com/...`)
- **C++:** Quoted includes (e.g., `#include "config.h"`)
- **Ruby:** `require_relative` paths

#### loc

Total lines of code. Counted via `content.lines().count()` on the source file.

### 1.4 Language-Specific Custom Fields

Custom fields are rendered as a nested section keyed by the parser's `language_id()`:

```yaml
rust:
  derives: [Debug, Clone, Serialize]
  unsafe_blocks: 1
  async_functions: 2
```

Fields within the section are sorted alphabetically and indented with two spaces. Values follow the same formatting rules as top-level fields: arrays use inline bracket syntax, numbers render directly, strings render without quotes.

The rendering logic in `formatter/mod.rs`:

```rust
if let Some((ref lang_id, ref fields)) = self.custom_fields {
    lines.push(format!("{}:", lang_id));
    let mut keys: Vec<&String> = fields.keys().collect();
    keys.sort();
    for key in keys {
        let value = &fields[key];
        lines.push(format!("  {}: {}", key, format_value(value)));
    }
}
```

#### Rust Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `derives` | array | `#[derive(...)]` macro arguments collected across all structs/enums |
| `unsafe_blocks` | integer | Count of `unsafe { ... }` blocks |
| `trait_impls` | array | `impl Trait for Type` relationships (e.g., `Display for Error`) |
| `lifetimes` | array | Lifetime parameters found (e.g., `'a`, `'static`) |
| `async_functions` | integer | Count of `async fn` declarations (both pub and private) |

#### Python Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `decorators` | array | Decorator names used (e.g., `staticmethod`, `property`, `cache`) |

#### Java Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `annotations` | array | Annotation names (e.g., `Service`, `Override`, `Deprecated`, `FunctionalInterface`) |

#### C++ Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `namespaces` | array | Namespace definitions found in the file |

#### C# Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `namespaces` | array | Namespace declarations (e.g., `MyApp.Services`, `MyApp.Models`) |
| `attributes` | array | Attribute names (e.g., `Serializable`, `Required`, `Obsolete`) |

#### Ruby Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `mixins` | array | Included/extended/prepended modules (e.g., `Comparable`, `Enumerable`) |

#### TypeScript/JavaScript and Go

No custom fields. These languages use only the core metadata fields.

---

## 2. Real Examples from Codebase Fixtures

The following examples show what fmm produces for each fixture file in `fixtures/`. These are derived from the actual source code and parser behavior.

### 2.1 Rust (`fixtures/sample.rs`)

Source contains: `pub struct Config` with `#[derive(Debug, Clone, Serialize, Deserialize)]`, `pub enum Status`, `pub struct Pipeline<'a>` with `'static` data, `pub struct Error` with `#[derive(Debug)]`, `impl Display for Error`, `pub fn process` with `unsafe` block, `pub(crate) fn internal_helper`, `pub(super) fn parent_visible`, and private `async fn fetch_remote`.

```yaml
file: fixtures/sample.rs
fmm: v0.2
exports: [Config, Error, Pipeline, Status, internal_helper, parent_visible, process]
imports: [anyhow, serde, tokio]
dependencies: [crate::config, super::utils]
loc: 61
modified: 2026-01-30
rust:
  async_functions: 1
  derives: [Clone, Debug, Deserialize, Serialize]
  lifetimes: ['a, 'static]
  trait_impls: [Display for Error]
  unsafe_blocks: 1
```

Notable behaviors:
- `pub(crate)` and `pub(super)` are treated as exports (they have pub visibility)
- `async fn fetch_remote` is not exported (no `pub`) but counted in `async_functions`
- `private_fn` is excluded entirely from exports
- Derives from `Config` (`Debug, Clone, Serialize, Deserialize`) and `Error` (`Debug`) are merged and deduplicated
- Lifetime `'a` from `Pipeline<'a>` and `'static` from `data: &'static str` are both captured
- `std::fmt` is excluded from imports (standard library filtering)

### 2.2 Python (`fixtures/sample.py`)

Source has `__all__`, classes with decorators, regular functions, relative imports, and private identifiers.

```yaml
file: fixtures/sample.py
fmm: v0.2
exports: [DataProcessor, MAX_RETRIES, ProcessConfig, fetch_data, transform]
imports: [pandas, pathlib, requests]
dependencies: [.utils, ..models]
loc: 51
modified: 2026-01-30
python:
  decorators: [property, staticmethod]
```

Notable behaviors:
- `__all__` controls exports: only `fetch_data`, `transform`, `DataProcessor`, `ProcessConfig`, `MAX_RETRIES` are listed
- `_internal_helper` and `_INTERNAL_TIMEOUT` are excluded (underscore prefix + not in `__all__`)
- `from .utils import helper` becomes dependency `.utils`
- `from ..models import User` becomes dependency `..models`
- Decorators `@staticmethod` and `@property` are collected from `DataProcessor`

### 2.3 Go (`fixtures/sample.go`)

Go uses capitalization to determine exports.

```yaml
file: fixtures/sample.go
fmm: v0.2
exports: [Config, Handler, MaxRetries, NewHandler, Process, Status, StatusActive, StatusInactive]
imports: [encoding/json, fmt, net/http]
dependencies: [github.com/gin-gonic/gin, github.com/redis/go-redis/v9]
loc: 69
modified: 2026-01-30
```

Notable behaviors:
- Capitalized identifiers are exports: `Config`, `Handler`, `MaxRetries`, `NewHandler`, `Process`, `Status`, `StatusActive`, `StatusInactive`
- Lowercase identifiers excluded: `internalTimeout`, `privateState`, `helperFunc`, `validate` (method receiver)
- Standard library packages (`encoding/json`, `fmt`, `net/http`) go into imports
- External modules (`github.com/gin-gonic/gin`, `github.com/redis/go-redis/v9`) go into dependencies
- No custom fields for Go

### 2.4 Java (`fixtures/sample.java`)

```yaml
file: fixtures/sample.java
fmm: v0.2
exports: [DataProcessor, ProcessConfig, Repository, Status]
imports: [java.util, org.springframework.stereotype]
loc: 58
modified: 2026-01-30
java:
  annotations: [Deprecated, FunctionalInterface, Override, Service]
```

Notable behaviors:
- Public classes (`DataProcessor`), interfaces (`Repository`, `ProcessConfig`), and enums (`Status`) are exports
- Private methods (`validate`) excluded
- Annotations from all public types collected: `@Service` on `DataProcessor`, `@Override` and `@Deprecated` on methods/types, `@FunctionalInterface` on `ProcessConfig`
- `java.util.List`, `java.util.Map`, `java.util.Optional` collapsed to `java.util`

### 2.5 C++ (`fixtures/sample.cpp`)

```yaml
file: fixtures/sample.cpp
fmm: v0.2
exports: [Config, Engine, Pipeline, Point, Status, process]
dependencies: [config.h, utils/helpers.h]
loc: 67
modified: 2026-01-30
cpp:
  namespaces: [engine, utils]
```

Notable behaviors:
- Classes (`Config`, `Engine`, `Pipeline`), structs (`Point`), enums (`Status`), and free functions (`process`) from namespace scope are exports
- `#include "config.h"` and `#include "utils/helpers.h"` are local dependencies (quoted includes)
- `#include <vector>` etc. are standard library and may be omitted or listed as imports
- Namespace names `engine` and `utils` captured as custom field
- Template class `Pipeline<T>` exported without the type parameter

### 2.6 C# (`fixtures/sample.cs`)

```yaml
file: fixtures/sample.cs
fmm: v0.2
exports: [DataService, IRepository, ProcessConfig, Status]
imports: [System, System.Collections.Generic, System.Linq, System.Threading.Tasks]
loc: 61
modified: 2026-01-30
csharp:
  attributes: [Obsolete, Required, Serializable]
  namespaces: [MyApp.Models, MyApp.Services]
```

Notable behaviors:
- Public classes (`DataService`, `ProcessConfig`), interfaces (`IRepository`), and enums (`Status`) are exports
- `InternalHelper` (marked `internal`) excluded from exports
- `using` statements become imports
- Both namespaces (`MyApp.Services`, `MyApp.Models`) captured
- Attributes `[Serializable]`, `[Required]`, `[Obsolete]` collected from all public types

### 2.7 Ruby (`fixtures/sample.rb`)

```yaml
file: fixtures/sample.rb
fmm: v0.2
exports: [Cacheable, DataProcessor, ProcessConfig, transform]
imports: [json, net/http]
dependencies: [config, lib/helpers]
loc: 65
modified: 2026-01-30
ruby:
  mixins: [Comparable, Enumerable]
```

Notable behaviors:
- Modules (`Cacheable`), classes (`DataProcessor`, `ProcessConfig`), and top-level methods (`transform`) are exports
- `_internal_helper` excluded (underscore prefix convention)
- `require 'json'` and `require 'net/http'` are imports
- `require_relative 'config'` and `require_relative 'lib/helpers'` are dependencies
- `include Comparable` and `include Enumerable` from `DataProcessor` captured as mixins

---

## 3. In-Memory Manifest

### 3.1 Purpose and Lifecycle

The manifest is an in-memory index built on-the-fly from all `*.fmm` sidecar files. It is **not persisted to disk** in v2.0 -- the legacy `.fmm/index.json` is no longer written. The manifest exists only while:

- The **MCP server** runs (rebuilt before each `tools/call` request)
- The **`search` command** executes (built once, queried, then discarded)

### 3.2 Rust Data Structure

```rust
// manifest/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,                           // "2.0"
    pub generated: DateTime<Utc>,                  // Build timestamp
    pub files: HashMap<String, FileEntry>,         // path -> metadata
    pub export_index: HashMap<String, String>,     // export_name -> file_path
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}
```

The `FileEntry` is derived from the parser's `Metadata` struct via a `From` implementation:

```rust
impl From<Metadata> for FileEntry {
    fn from(metadata: Metadata) -> Self {
        Self {
            exports: metadata.exports,
            imports: metadata.imports,
            dependencies: metadata.dependencies,
            loc: metadata.loc,
        }
    }
}
```

### 3.3 JSON Serialization Schema

When serialized (for MCP tool responses), the manifest uses camelCase due to `#[serde(rename_all = "camelCase")]`:

```json
{
  "version": "2.0",
  "generated": "2026-01-30T12:00:00Z",
  "files": {
    "src/auth/session.ts": {
      "exports": ["createSession", "validateSession"],
      "imports": ["jwt", "redis"],
      "dependencies": ["./types", "./config"],
      "loc": 234
    },
    "src/utils/crypto.ts": {
      "exports": ["encrypt", "decrypt"],
      "imports": ["crypto"],
      "dependencies": ["./types"],
      "loc": 89
    }
  },
  "exportIndex": {
    "createSession": "src/auth/session.ts",
    "validateSession": "src/auth/session.ts",
    "encrypt": "src/utils/crypto.ts",
    "decrypt": "src/utils/crypto.ts"
  }
}
```

Key points:
- `export_index` becomes `exportIndex` in JSON
- File paths as HashMap keys use forward slashes (platform-normalized)
- `FileEntry` does not include `modified` or custom fields -- those exist only in sidecars

### 3.4 Export Index: O(1) Symbol Lookup

The `export_index` is a reverse lookup table: given an export name, return the file that defines it. This is the manifest's primary value for LLMs.

#### Construction

When a file is added to the manifest, each export name is inserted:

```rust
for export in &metadata.exports {
    manifest.export_index.insert(export.clone(), key.clone());
}
```

#### Conflict Resolution: .ts/.tsx Priority

When the same export name exists in both a `.ts`/`.tsx` file and a `.js`/`.jsx` file, TypeScript wins:

```rust
let should_insert = match self.export_index.get(export) {
    None => true,
    Some(existing) => {
        let existing_is_ts = existing.ends_with(".ts") || existing.ends_with(".tsx");
        let new_is_js = path.ends_with(".js") || path.ends_with(".jsx");
        !(existing_is_ts && new_is_js)
    }
};
```

This handles the common case of compiled TypeScript projects where both `foo.ts` and `foo.js` exist with identical exports. The `.ts` file is the source; the `.js` file is the build artifact.

#### Stale Export Cleanup

When a file is updated via `add_file()`, old exports that no longer exist are removed before new ones are inserted:

```rust
if let Some(old_entry) = self.files.get(path) {
    for old_export in &old_entry.exports {
        if self.export_index.get(old_export) == Some(&path.to_string()) {
            self.export_index.remove(old_export);
        }
    }
}
```

The conditional check (`== Some(&path.to_string())`) prevents accidentally removing an export that was reassigned to a different file (e.g., if `foo` was moved from `a.ts` to `b.ts` and both files are being processed).

### 3.5 Manifest Building from Sidecars

`Manifest::load_from_sidecars(root)` walks the directory tree using the `ignore` crate (automatically respecting `.gitignore`), finds all `*.fmm` files, parses each one, and builds the index:

```rust
pub fn load_from_sidecars(root: &Path) -> Result<Self> {
    let mut manifest = Self::new();
    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("fmm") {
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,   // silently skip unreadable sidecars
        };
        if let Some((file_path, entry)) = parse_sidecar(&content) {
            let key = if !file_path.is_empty() {
                file_path               // use path from sidecar content
            } else {
                // fallback: derive from sidecar filesystem path
                let source_path = path.with_extension("");
                source_path.strip_prefix(root)
                    .unwrap_or(&source_path)
                    .display().to_string()
            };
            for export in &entry.exports {
                manifest.export_index.insert(export.clone(), key.clone());
            }
            manifest.files.insert(key, entry);
        }
    }
    Ok(manifest)
}
```

The file path stored in the manifest comes from the sidecar's `file:` field (not from the sidecar's filesystem path). This ensures consistency regardless of how the sidecar is discovered.

### 3.6 Validation

The manifest supports file validation to check if sidecars are current:

```rust
pub fn validate_file(&self, path: &str, current: &Metadata) -> bool {
    if let Some(entry) = self.files.get(path) {
        entry.exports == current.exports
            && entry.imports == current.imports
            && entry.dependencies == current.dependencies
            && entry.loc == current.loc
    } else {
        false
    }
}
```

Validation compares all four metadata fields. A mismatch in any field -- even LOC changing by one line -- marks the sidecar as stale.

---

## 4. Configuration File (.fmmrc.json)

### 4.1 Schema

```json
{
  "languages": ["ts", "tsx", "js", "jsx", "py", "rs", "go"],
  "format": "yaml",
  "include_loc": true,
  "include_complexity": false,
  "max_file_size": 1024
}
```

### 4.2 Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `languages` | string array | `["ts", "tsx", "js", "jsx", "py", "rs", "go"]` | File extensions to process |
| `format` | `"yaml"` or `"json"` | `"yaml"` | Sidecar output format |
| `include_loc` | boolean | `true` | Include line counts in sidecars |
| `include_complexity` | boolean | `false` | Reserved for future cyclomatic complexity |
| `max_file_size` | integer (KB) | `1024` | Skip files larger than this |

### 4.3 All Supported Extensions

| Parser | Extensions | Language ID |
|--------|-----------|-------------|
| TypeScript/JS | `ts`, `tsx`, `js`, `jsx` | `typescript` |
| Python | `py` | `python` |
| Rust | `rs` | `rust` |
| Go | `go` | `go` |
| Java | `java` | `java` |
| C++ | `cpp`, `hpp`, `cc`, `hh`, `cxx`, `hxx` | `cpp` |
| C# | `cs` | `csharp` |
| Ruby | `rb` | `ruby` |

The default configuration only enables `ts`, `tsx`, `js`, `jsx`, `py`, `rs`, `go`. To use Java/C++/C#/Ruby, add their extensions to the `languages` array.

### 4.4 Loading Behavior

Configuration is loaded from `.fmmrc.json` in the current working directory. If the file doesn't exist, all defaults are used:

```rust
pub fn load() -> Result<Self> {
    let path = Path::new(".fmmrc.json");
    if !path.exists() {
        return Ok(Self::default());
    }
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}
```

Missing fields in the JSON fall back to their serde defaults, so a partial config like `{"languages": ["rs"]}` is valid.

---

## 5. Design Decisions

### Why YAML-like, Not Strict YAML?

The sidecar format looks like YAML but is parsed with simple prefix matching, not a YAML library. This gives:

1. **No external dependency** on a YAML parser for reading sidecars
2. **Single-pass parsing** -- O(n) where n = lines in sidecar
3. **Inline arrays** are more compact than YAML block syntax
4. **No ambiguity** from YAML's complex type coercion rules (no `Yes`/`No` booleans, no `1.0` as float vs string)

The trade-off: custom fields in nested sections cannot use arbitrary YAML features. This is acceptable because nesting is always exactly one level deep with known value types.

### Why Inline Arrays?

```yaml
exports: [createSession, validateSession]    # fmm: 1 line
```

vs.

```yaml
exports:                                      # standard YAML block: 3 lines
  - createSession
  - validateSession
```

Inline is 1 line vs. 3 lines. For sidecars that exist primarily for machine consumption, density matters. An LLM reads fewer tokens to get the same information.

### Why file: First?

When an LLM reads a sidecar -- or when batch-reading multiple sidecars -- the first line immediately identifies which source file this metadata describes. This orientation is critical for navigation workflows.

### Why Omit Empty Fields?

If a file has no imports, the `imports:` line is not rendered at all (rather than `imports: []`). This keeps sidecars minimal. A TypeScript file with no external imports and no local dependencies produces:

```yaml
file: src/types.ts
fmm: v0.2
exports: [UserType, SessionType]
loc: 12
modified: 2026-01-30
```

Four lines instead of seven. Every omitted line saves tokens.

### Why No Persisted Manifest?

v1 wrote `.fmm/index.json` to disk. v2 builds the manifest in memory from sidecars. Reasons:

1. **Single source of truth** -- sidecars are the authority, manifest is derived
2. **No sync problems** -- can't have a stale manifest if it's always rebuilt
3. **Git-clean** -- no generated JSON file to track or merge
4. **Fast enough** -- loading from sidecars takes <100ms for typical projects

The MCP server rebuilds the manifest before every `tools/call` request (line 104 of `mcp/mod.rs`), ensuring queries always reflect current sidecar state.

---

## 6. Parsing Pipeline

The complete data flow from source file to sidecar:

```
source.ts
    |
    v
FileProcessor.generate(path)
    |
    v
ParserRegistry.get_parser("ts") -> TypeScriptParser
    |
    v
TypeScriptParser.parse(source) -> ParseResult {
    metadata: Metadata { exports, imports, dependencies, loc },
    custom_fields: None  // TS has no custom fields
}
    |
    v
Frontmatter::new(relative_path, metadata)
    .with_version("v0.2")
    .with_custom_fields(language_id, custom_fields)
    .render()
    |
    v
"file: src/source.ts\nfmm: v0.2\nexports: [...]\n..."
    |
    v
fs::write("src/source.ts.fmm", rendered + "\n")
```

All parsing uses tree-sitter for AST construction. Each language parser runs a single tree-sitter parse pass and extracts exports, imports, dependencies, LOC, and custom fields in one walk of the syntax tree. No file is parsed twice.

The `format_sidecar` method in `extractor/mod.rs` handles path normalization, version stamping, and custom field attachment:

```rust
fn format_sidecar(&self, path: &Path, metadata: &Metadata,
                   custom_fields: Option<&HashMap<String, serde_json::Value>>) -> Result<String> {
    let relative_path = path.strip_prefix(&self.root).unwrap_or(path);
    let language_id = self.registry.get_parser(extension)
        .ok().map(|p| p.language_id().to_string());

    let frontmatter = Frontmatter::new(relative_path.display().to_string(), metadata.clone())
        .with_version("v0.2")
        .with_custom_fields(language_id.as_deref(), custom_fields);

    Ok(format!("{}\n", frontmatter.render()))
}
```

---

## 7. Format Value Rendering

The `format_value` function in `formatter/mod.rs` handles all JSON value types for custom field rendering:

| JSON Type | Rendered As | Example |
|-----------|------------|---------|
| Array | `[item1, item2]` | `[Debug, Clone]` |
| String | bare string | `Display for Error` |
| Number | bare number | `3` |
| Boolean | `true`/`false` | `true` |
| Null | `null` | `null` |
| Object | `{key: val, ...}` | `{name: test}` |

String values within arrays are rendered without quotes. This is deliberate -- the format prioritizes readability over strict YAML compliance.

---

## 8. Update and Validation Semantics

### Smart Update Detection

The `update` command does not blindly overwrite sidecars. It regenerates the sidecar content and compares it to the existing file:

```rust
pub fn update(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
    let new_yaml = self.format_sidecar(path, &result.metadata, ...)?;
    let sidecar = sidecar_path_for(path);
    if sidecar.exists() {
        let old = fs::read_to_string(&sidecar)?;
        if old.trim() == new_yaml.trim() {
            return Ok(None);  // no change needed
        }
    }
    // ... write new content
}
```

This means `fmm update` is safe to run repeatedly -- it only touches files whose metadata has actually changed.

### Validation Semantics

`fmm validate` re-parses each source file, regenerates the expected sidecar content, and compares it character-for-character (after trimming) against the existing sidecar:

```rust
pub fn validate(&self, path: &Path) -> Result<bool> {
    let expected = self.format_sidecar(path, &result.metadata, ...)?;
    let actual = fs::read_to_string(&sidecar)?;
    Ok(actual.trim() == expected.trim())
}
```

Validation fails if:
- The sidecar file doesn't exist
- Any metadata field differs (exports added/removed, imports changed, LOC changed)
- Custom fields changed (new derives, different decorator list)
- The `modified` date differs (since it reflects generation date, not source modification)

---

This document specifies the complete data format contract for fmm. Any tool that reads `.fmm` sidecar files or queries the manifest can use this as the authoritative reference.
