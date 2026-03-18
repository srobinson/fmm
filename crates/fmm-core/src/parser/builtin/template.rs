//! Template for adding a new language parser to fmm.
//!
//! **Do not modify this file.** Copy it to `src/parser/builtin/<lang>.rs` and
//! follow the instructions in `docs/CONTRIBUTING_LANGUAGE.md`.
//!
//! This module is gated with `#[cfg(any())]` in `mod.rs` and is never compiled.
//! It exists solely as a fill-in-the-blanks reference.
//!
//! Capture naming conventions are documented in `docs/QUERIES.md`.

// ─── Imports ───────────────────────────────────────────────────────────────
// Keep only what your parser actually uses.
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashSet;
use tree_sitter::{Language, Parser as TSParser};

// If using tree-sitter queries (recommended for most languages), also add:
// use super::query_helpers::{collect_matches_with_lines, compile_query, make_parser};
// use tree_sitter::Query;

// ─── Parser struct ─────────────────────────────────────────────────────────

/// Replace `Template` with `YourLang` (e.g., `HaskellParser`, `ErlangParser`).
pub struct TemplateParser {
    parser: TSParser,
    // If using compiled queries, add them as fields here:
    // exports_query: Query,
    // imports_query: Query,
}

impl TemplateParser {
    pub fn new() -> Result<Self> {
        // STEP 1: Replace with your grammar crate's LANGUAGE constant.
        //   Add the crate to Cargo.toml first (e.g., `tree-sitter-haskell = "..."`).
        //   Examples:
        //     tree_sitter_python::LANGUAGE.into()
        //     tree_sitter_go::LANGUAGE.into()
        //     tree_sitter_lua::LANGUAGE.into()
        let language: Language = todo!("Replace with: tree_sitter_LANG::LANGUAGE.into()");

        // Use make_parser() instead of the raw TSParser::new() + set_language() block.
        let parser = make_parser(&language, "LANG")?;

        // STEP 2 (optional): Compile your S-expression queries here.
        //   Use compile_query() instead of Query::new().map_err().
        //   See docs/QUERIES.md for capture name conventions.
        //
        //   let exports_query = compile_query(
        //       &language,
        //       r#"
        //           (function_definition
        //               name: (identifier) @name)
        //       "#,
        //       "exports",
        //   )?;

        Ok(Self { parser })
    }

    // ─── Export extraction ────────────────────────────────────────────────

    /// Walk the AST and return all exported symbols with their source ranges.
    ///
    /// Two common patterns — pick one:
    ///
    /// **A. Cursor walk** (simpler, good for small ASTs — see lua.rs, zig.rs):
    ///   Iterate `root_node.children()`, match on `child.kind()`, extract identifier text.
    ///
    /// **B. Tree-sitter queries** (cleaner for complex grammars — see typescript.rs, python.rs):
    ///   Use `collect_matches_with_lines(&self.exports_query, root_node, source_bytes)`
    ///   from `super::query_helpers`.
    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let _source_bytes = source.as_bytes();
        let mut exports = Vec::new();

        // TODO: Implement export extraction.
        //
        // Cursor-walk skeleton (adapt node kinds for your language's AST):
        //
        //   let mut seen = HashSet::new();
        //   let mut cursor = root_node.walk();
        //   for child in root_node.children(&mut cursor) {
        //       if child.kind() == "function_declaration" && is_public(&child, _source_bytes) {
        //           if let Some(name) = extract_identifier(&child, _source_bytes) {
        //               if seen.insert(name.clone()) {
        //                   exports.push(ExportEntry::new(
        //                       name,
        //                       child.start_position().row + 1,
        //                       child.end_position().row + 1,
        //                   ));
        //               }
        //           }
        //       }
        //   }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    // ─── Import extraction ────────────────────────────────────────────────

    /// Return `(imports, dependencies)` where:
    ///   - `imports`      = external packages and standard library modules
    ///   - `dependencies` = relative file paths (start with `.`)
    ///
    /// See lua.rs (`walk_requires`), zig.rs (`walk_imports`), or
    /// typescript.rs (`extract_imports_from_tree`) for complete examples.
    fn extract_imports(
        &self,
        _source: &str,
        _root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        // TODO: Implement import extraction.
        //
        // Walk the tree looking for import statements (syntax is language-specific).
        // Classify each resolved path:
        //   starts_with('.') -> dependencies
        //   otherwise        -> imports
        //
        // The distinction drives the dependency graph that fmm builds.

        let mut imports: Vec<String> = Vec::new();
        let mut dependencies: Vec<String> = Vec::new();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }
}

// ─── Parser trait implementation ───────────────────────────────────────────

impl Parser for TemplateParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse LANG source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let loc = source.lines().count();

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
                ..Default::default()
            },
            // Set to Some(HashMap) only when your language has extra sidecar fields.
            // See zig.rs (comptime_blocks, test_blocks) for an example.
            custom_fields: None,
        })
    }

    fn language_id(&self) -> &'static str {
        // The identifier written into sidecar frontmatter (lowercase, no spaces).
        // Examples: "haskell", "erlang", "cobol"
        "LANG"
    }

    fn extensions(&self) -> &'static [&'static str] {
        // All file extensions handled by this parser (without the leading dot).
        // Examples: &["hs", "lhs"]  or  &["erl"]  or  &["cbl", "cob"]
        &["ext"]
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Rename and adapt these for your language.
    // Aim for at least:
    //   - one test that verifies exported symbols are detected
    //   - one test that verifies private/unexported symbols are excluded
    //   - one test for import/dependency extraction
    //   - one test for an empty file

    #[test]
    fn parse_LANG_exports() {
        let mut parser = TemplateParser::new().unwrap();
        // TODO: Replace with a minimal LANG snippet that defines one exported symbol.
        let source = "";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"my_exported_symbol".to_string()));
    }

    #[test]
    fn parse_LANG_empty() {
        let mut parser = TemplateParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
