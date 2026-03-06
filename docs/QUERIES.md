# Tree-sitter Query Conventions

This document defines the capture names shared across all fmm language parsers and explains how to write and debug tree-sitter queries.

See also: `docs/CONTRIBUTING_LANGUAGE.md` for the full step-by-step guide.

---

## Standard Capture Names

Every capture name used in fmm queries has a fixed meaning. Follow these conventions in any new parser so the shared helpers in `src/parser/builtin/query_helpers.rs` work correctly.

| Capture | Meaning | Example usage |
|---|---|---|
| `@name` | Primary identifier being defined | Function name, class name, variable name |
| `@vis` | Visibility modifier node | `pub` (Rust), `public` (Java/Kotlin), `export` (JS/TS) |
| `@source` | Import/export source path | `from './utils'` (TS), the path string in `import "..."` (Go) |
| `@class_name` | Parent class identifier (for method extraction) | Class name when a method is nested inside |
| `@method_name` | Method identifier (paired with `@class_name`) | Method name under a class or impl block |
| `@attr_name` | Decorator or annotation name | `@derive` (Rust), `@dataclass` (Python), `@Override` (Java) |
| `@original_name` | Pre-alias export name | TS: `export { foo as bar }` captures `foo` as `@original_name` |
| `@values` | List literal (for `__all__`-style explicit export lists) | Python `__all__ = ["foo", "bar"]` |

**Rule:** If a capture is a bare identifier being defined, use `@name`. All other captures describe modifiers or context around that identifier.

---

## S-expression Query Basics

Tree-sitter queries use S-expression patterns that mirror the AST structure. To write a query:

```scheme
; Match a top-level function declaration and capture its name:
(function_declaration
    name: (identifier) @name)

; Match only exported functions (TypeScript/JavaScript):
(export_statement
    (function_declaration
        name: (identifier) @name))

; Match imports with a source path:
(import_statement
    source: (string) @source)

; Named field access uses `field_name: (node_type)`:
(assignment
    left: (identifier) @name
    right: (_) @value)
```

Predicates filter matches after the pattern matches:

```scheme
; Only match identifiers starting with uppercase:
((identifier) @name
    (#match? @name "^[A-Z]"))
```

---

## Discovering Node Type Names

Before writing a query, you need to know what the AST looks like for your language.

### With the tree-sitter CLI

```bash
# Install the CLI
cargo install tree-sitter-cli

# Inspect the AST for a source snippet
echo 'def hello(): pass' | tree-sitter parse --language python /dev/stdin

# Or parse a file
tree-sitter parse path/to/sample.py
```

The output shows node kinds and field names, e.g.:

```
(module [0, 0] - [1, 0]
  (function_definition [0, 0] - [0, 16]
    name: (identifier) [0, 4] - [0, 9]
    parameters: (parameters [0, 9] - [0, 11])
    body: (block [0, 11] - [0, 16]
      (pass_statement [0, 13] - [0, 17]))))
```

The `name:` prefix is the field name; `(identifier)` is the node kind. Your query would be:

```scheme
(function_definition
    name: (identifier) @name)
```

### Testing queries interactively

```bash
# Test a query against source code
tree-sitter query '(function_definition name: (identifier) @name)' sample.py
```

This prints each match and captured node so you can verify correctness before writing Rust.

---

## Helper Functions in `query_helpers.rs`

`src/parser/builtin/query_helpers.rs` provides three helpers that cover the majority of use cases:

| Function | Returns | Use when |
|---|---|---|
| `collect_matches(query, root, bytes)` | `Vec<String>` | Simple: all captures from any capture name, deduplicated |
| `collect_named_matches(query, "name", root, bytes)` | `Vec<String>` | When a query has multiple capture names and you want only one |
| `collect_matches_with_lines(query, root, bytes)` | `Vec<ExportEntry>` | When you need start/end line ranges (always prefer this for exports) |

`collect_matches_with_lines` calls `top_level_ancestor(node)` to expand each capture up to the enclosing top-level declaration, so the line range covers the full definition rather than just the identifier token.

---

## Cursor-Walk vs Query Approach

Two styles are in use across the parser suite. Choose based on what's cleaner for your language:

**Cursor walk** — iterate `root_node.children()` and recurse manually.

- Good when: visibility is determined by a sibling keyword (`local`, `pub`), or the grammar has irregular structure.
- Examples: `lua.rs`, `zig.rs`.

**Tree-sitter queries** — write S-expression patterns and use the helpers above.

- Good when: the grammar has consistent, predictable structure (most languages).
- Examples: `typescript.rs`, `python.rs`, `go.rs`.

The two approaches can be mixed within a single parser file if different features call for it.
