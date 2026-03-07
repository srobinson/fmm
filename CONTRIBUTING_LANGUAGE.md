# Adding a Language Parser

This guide walks through adding a new language to fmm in eight steps. A contributor with no prior fmm experience should be able to complete this in under 30 minutes.

**Reference:** All parsers live in `src/parser/builtin/`. `src/parser/builtin/lua.rs` (~275 LOC) is the simplest complete implementation. Start there if you want to read existing code before writing your own.

---

## Prerequisites

- Rust toolchain (`cargo build` passes)
- A `tree-sitter-<lang>` crate published on [crates.io](https://crates.io) for your target language

---

## The 8 Steps

### Step 1 — Find the grammar crate

Search [crates.io](https://crates.io) for `tree-sitter-<lang>`. Most languages have one. Check that it:

- Exports a `LANGUAGE` constant (newer crates) or a `language()` function (older crates)
- Has a version that is recent and maintained

Common pattern for modern crates:

```toml
tree-sitter-haskell = "0.23"
```

If the crate exposes `language()` instead of `LANGUAGE`, convert it:

```rust
// Old API:
let language = tree_sitter_haskell::language();
// New API (preferred):
let language: Language = tree_sitter_haskell::LANGUAGE.into();
```

### Step 2 — Add the crate to `Cargo.toml`

Under `[dependencies]`:

```toml
tree-sitter-haskell = "0.23"
```

Run `cargo build` to pull and compile the grammar.

### Step 3 — Create `src/parser/builtin/<lang>.rs`

Copy the template:

```bash
cp src/parser/builtin/template.rs src/parser/builtin/haskell.rs
```

Open `haskell.rs` and make these substitutions throughout:

| Placeholder | Replace with |
|---|---|
| `TemplateParser` | `HaskellParser` |
| `tree_sitter_LANG::LANGUAGE` | `tree_sitter_haskell::LANGUAGE` |
| `"LANG"` | `"haskell"` |
| `&["ext"]` | `&["hs", "lhs"]` |
| `LANG` in error strings | `Haskell` |

Then implement `extract_exports()` and `extract_imports()`. See `docs/QUERIES.md` for:

- How to discover node type names with the tree-sitter CLI
- Standard capture name conventions (`@name`, `@vis`, `@source`, etc.)
- When to use cursor-walk vs tree-sitter queries

**Choosing an approach:**

- If visibility is a sibling keyword (like `pub` or `local`), use a cursor walk — see `lua.rs` or `zig.rs`.
- If the grammar has consistent structure, use tree-sitter queries — see `typescript.rs` or `python.rs`.

Minimal cursor-walk skeleton:

```rust
fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
    let source_bytes = source.as_bytes();
    let mut seen = std::collections::HashSet::new();
    let mut exports = Vec::new();
    let mut cursor = root_node.walk();

    for child in root_node.children(&mut cursor) {
        if child.kind() == "function_declaration" {
            // TODO: check visibility (is_pub, has_export_keyword, etc.)
            if let Some(name) = extract_identifier(&child, source_bytes) {
                if seen.insert(name.clone()) {
                    exports.push(ExportEntry::new(
                        name,
                        child.start_position().row + 1,
                        child.end_position().row + 1,
                    ));
                }
            }
        }
    }

    exports.sort_by_key(|e| e.start_line);
    exports
}
```

### Step 4 — Export the module from `src/parser/builtin/mod.rs`

Add one line in alphabetical order:

```rust
pub mod haskell;
```

### Step 5 — Register the parser in `src/parser/mod.rs`

Inside `ParserRegistry::register_builtin()`, add two calls:

```rust
// Haskell
self.register(&["hs", "lhs"], || {
    Ok(Box::new(builtin::haskell::HaskellParser::new()?))
});
self.register_language_id(&["hs", "lhs"], "haskell");
```

Place the block near other single-extension languages (alphabetical order by language name is fine).

### Step 6 — Create `fixtures/sample.<ext>`

Add a realistic sample file (50–150 lines of real code that exercises the features your parser extracts). The fixture must contain:

- At least one exported function or class
- At least one private/unexported symbol
- At least one import statement (if the language supports them)

Example for Haskell: `fixtures/sample.hs`

```haskell
module Sample
    ( greet
    , Config(..)
    ) where

import Data.Text (Text)
import qualified Data.Map as Map
import System.IO (hPutStrLn, stderr)

data Config = Config
    { configName :: Text
    , configPort :: Int
    }

greet :: Config -> Text
greet cfg = "Hello, " <> configName cfg

-- private helper, not exported
formatPort :: Int -> String
formatPort p = ":" <> show p
```

### Step 7 — Generate the index

```bash
cargo run -- generate fixtures/
```

Then query the index to verify:

```bash
cargo run -- outline fixtures/sample.hs
```

Check that:

- Exports list the symbols you expected
- Imports and dependencies are correct
- No obvious omissions or false positives

If the output looks wrong, go back to Step 3 and adjust your extraction logic.

### Step 8 — Add a fixture validation test

Open `tests/fixture_validation.rs` and add a `validate_haskell_fixture()` test following the pattern of existing tests in that file. At minimum, assert:

- The expected exports appear in the right order
- At least one known import is present
- Known private symbols are absent
- `loc` is non-zero

```rust
#[test]
fn validate_haskell_fixture() {
    let source = include_str!("../fixtures/sample.hs");
    let mut parser = HaskellParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result.metadata.export_names().contains(&"greet".to_string()));
    assert!(!result.metadata.export_names().contains(&"formatPort".to_string()));
    assert!(result.metadata.imports.contains(&"Data.Text".to_string()));
    assert!(result.metadata.loc > 10);
}
```

Run the full test suite to confirm nothing regressed:

```bash
just test
```

---

## Checklist

```
[ ] 1. tree-sitter-<lang> crate identified on crates.io
[ ] 2. Crate added to Cargo.toml; cargo build passes
[ ] 3. src/parser/builtin/<lang>.rs created from template; extract_exports() and extract_imports() implemented
[ ] 4. pub mod <lang>; added to src/parser/builtin/mod.rs
[ ] 5. Parser registered in ParserRegistry::register_builtin() in src/parser/mod.rs
[ ] 6. fixtures/sample.<ext> created with 50-150 LOC of real code
[ ] 7. cargo run -- generate fixtures/ && cargo run -- outline fixtures/sample.<ext> shows correct exports
[ ] 8. validate_<lang>_fixture() test added to tests/fixture_validation.rs; just test passes
```

---

## Getting help

- Capture naming conventions: `docs/QUERIES.md`
- Simplest reference implementation: `src/parser/builtin/lua.rs`
- Query-based reference implementation: `src/parser/builtin/typescript.rs`
- Shared query utilities: `src/parser/builtin/query_helpers.rs`
