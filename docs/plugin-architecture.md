# FMM Plugin Architecture

## Overview

External plugins allow third-party language parsers to integrate with fmm without modifying core code. Plugins are dynamic libraries (.dylib/.so/.dll) that expose a C FFI interface.

## Plugin Discovery

Plugins are discovered from:
1. `~/.fmm/plugins/` (user-global plugins)
2. `.fmm/plugins/` (project-local plugins)
3. `FMM_PLUGIN_PATH` environment variable (colon-separated paths)

Project-local plugins take precedence over global ones for the same extension.

## Plugin Metadata

Each plugin must expose a metadata function:

```c
typedef struct {
    const char* name;           // e.g., "fmm-plugin-go"
    const char* version;        // semver, e.g., "0.1.0"
    const char* language_id;    // e.g., "go" (used in frontmatter sections)
    const char* const* extensions; // null-terminated array: ["go", NULL]
    uint32_t fmm_api_version;  // must match host API version
} FmmPluginMetadata;
```

## FFI Interface

Plugins expose these `extern "C"` functions:

```c
// Required: Return plugin metadata.
// Called once at load time. The returned pointer must remain valid
// for the lifetime of the plugin.
FmmPluginMetadata* fmm_plugin_metadata(void);

// Required: Parse source code and return metadata as JSON.
// The returned string is owned by the plugin; the host will call
// fmm_free_string() when done.
// Returns NULL on parse error.
const char* fmm_plugin_parse(const char* source, uint32_t source_len);

// Required: Free a string previously returned by fmm_plugin_parse.
void fmm_free_string(const char* ptr);

// Optional: Return custom fields as JSON.
// Same ownership semantics as fmm_plugin_parse.
// Return NULL if no custom fields.
const char* fmm_plugin_custom_fields(const char* source, uint32_t source_len);
```

### Return Format

`fmm_plugin_parse` returns JSON:

```json
{
    "exports": ["foo", "bar"],
    "imports": ["external_pkg"],
    "dependencies": ["./local_mod"],
    "loc": 150
}
```

`fmm_plugin_custom_fields` returns JSON:

```json
{
    "goroutines": 3,
    "channels": ["dataCh", "errCh"]
}
```

## Safety Constraints

### Sandboxing
- Plugins run in the host process (no subprocess isolation for v1)
- Future: consider WASM-based sandboxing for untrusted plugins
- Plugin directory permissions should be restricted (0755)

### Timeouts
- Parse operations have a 5-second timeout per file
- Plugins that exceed the timeout are unloaded and blacklisted for the session
- Custom fields extraction has a separate 2-second timeout

### Validation
- `fmm_api_version` must match the host's API version exactly
- Plugin metadata is validated at load time (non-null fields, valid extensions)
- Duplicate extensions: project-local wins over global, last-loaded wins within same scope

### Error Handling
- NULL returns from parse are treated as "file not parseable" (skip gracefully)
- Panics in plugin code are caught at the FFI boundary
- Invalid JSON returns are logged and treated as parse failures

## Rust Implementation Sketch

```rust
// In src/parser/plugin.rs

use libloading::{Library, Symbol};
use std::path::Path;

pub struct PluginParser {
    _lib: Library,  // Must outlive the function pointers
    parse_fn: Symbol<'static, extern "C" fn(*const u8, u32) -> *const i8>,
    free_fn: Symbol<'static, extern "C" fn(*const i8)>,
    custom_fields_fn: Option<Symbol<'static, extern "C" fn(*const u8, u32) -> *const i8>>,
    language_id: String,
}

impl Parser for PluginParser {
    fn parse(&mut self, source: &str) -> Result<Metadata> {
        let ptr = (self.parse_fn)(source.as_ptr(), source.len() as u32);
        if ptr.is_null() {
            anyhow::bail!("Plugin parse returned null");
        }
        let json_str = unsafe { CStr::from_ptr(ptr) }.to_str()?;
        let metadata: Metadata = serde_json::from_str(json_str)?;
        (self.free_fn)(ptr);
        Ok(metadata)
    }

    fn language_id(&self) -> &'static str {
        // Leak for 'static lifetime (plugins live for program duration)
        Box::leak(self.language_id.clone().into_boxed_str())
    }

    fn extensions(&self) -> &'static [&'static str] {
        // Populated from metadata at load time
        &[]
    }
}
```

## Plugin Development Guide

### Minimal Go Plugin Example

```go
package main

import "C"
import (
    "encoding/json"
    "unsafe"
)

//export fmm_plugin_metadata
func fmm_plugin_metadata() *C.FmmPluginMetadata {
    // Return metadata struct
}

//export fmm_plugin_parse
func fmm_plugin_parse(source *C.char, sourceLen C.uint) *C.char {
    goSource := C.GoStringN(source, C.int(sourceLen))
    // Parse Go source code...
    result, _ := json.Marshal(metadata)
    return C.CString(string(result))
}

//export fmm_free_string
func fmm_free_string(ptr *C.char) {
    C.free(unsafe.Pointer(ptr))
}
```

Build: `go build -buildmode=c-shared -o fmm-plugin-go.so`

### Testing Plugins

```bash
# Place plugin in discovery path
cp fmm-plugin-go.so ~/.fmm/plugins/

# Verify discovery
fmm status  # Should list "go" in supported languages

# Test on a Go project
fmm generate --dry-run ./my-go-project
```

## Versioning

- `fmm_api_version: 1` — initial plugin API (this document)
- Breaking changes increment the API version
- Plugins with mismatched versions are skipped with a warning
