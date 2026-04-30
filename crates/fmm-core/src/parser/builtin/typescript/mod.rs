mod extract_classes;
mod extract_imports;
#[cfg(test)]
mod tests;
pub(crate) mod tsconfig;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::{collect_matches_with_lines, compile_query, make_parser};

use tsconfig::load_tsconfig_paths;

pub struct TypeScriptParser {
    parser: TSParser,
    export_queries: Vec<Query>,
    import_query: Query,
    /// ALP-749/750: captures source string from `export { X } from '...'` and `export * from '...'`
    reexport_source_query: Query,
    /// ALP-754: `@Foo` style decorators
    decorator_query: Query,
    /// ALP-754: `@Foo()` call-expression style decorators
    call_decorator_query: Query,
    /// ALP-768: captures class declarations for public method extraction
    class_query: Query,
    /// ALP-881: captures named import specifiers with their source path
    named_import_query: Query,
    /// ALP-881: captures namespace import source paths (`import * as X from '...'`)
    namespace_import_query: Query,
    /// ALP-881: captures named re-export specifiers (`export { foo } from '...'`)
    reexport_named_query: Query,
    /// true when built for TSX/JSX (ALP-753)
    is_tsx: bool,
    /// ALP-925: per-parser cache of file directory → resolved tsconfig path aliases.
    /// Populated lazily; avoids redundant filesystem walks for files sharing a directory.
    tsconfig_cache: HashMap<std::path::PathBuf, HashMap<String, Vec<String>>>,
}

impl TypeScriptParser {
    pub fn new() -> Result<Self> {
        Self::build(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), false)
    }

    /// Parser for `.tsx` and `.jsx` files — uses `LANGUAGE_TSX` (ALP-753).
    pub fn new_tsx() -> Result<Self> {
        Self::build(tree_sitter_typescript::LANGUAGE_TSX.into(), true)
    }

    fn build(language: Language, is_tsx: bool) -> Result<Self> {
        let parser = make_parser(&language, "TypeScript")?;

        let export_query_strs = [
            "(export_statement (function_declaration name: (identifier) @name))",
            "(export_statement (lexical_declaration (variable_declarator name: (identifier) @name)))",
            "(export_statement (class_declaration name: (type_identifier) @name))",
            "(export_statement (interface_declaration name: (type_identifier) @name))",
            // ALP-752: capture alias when present (export { foo as bar } → bar)
            "(export_statement (export_clause (export_specifier alias: (identifier) @name)))",
            // ALP-752: capture name only when no alias is present (export { foo } → foo)
            "(export_statement (export_clause (export_specifier !alias name: (identifier) @name)))",
            // export type Foo = { ... }
            "(export_statement (type_alias_declaration name: (type_identifier) @name))",
            // export default SomeIdentifier
            "(export_statement value: (identifier) @name)",
            // ALP-751: export enum Direction {} / export const enum Status {}
            "(export_statement (enum_declaration name: (identifier) @name))",
            // ALP-755: export * as ns from './mod'
            "(export_statement (namespace_export (identifier) @name))",
            // ALP-756: export namespace Foo {} (uses `internal_module` in tree-sitter-typescript)
            "(export_statement (internal_module name: (identifier) @name))",
            // ALP-756: export module Foo {} (uses `module` node)
            "(export_statement (module name: (identifier) @name))",
        ];

        let export_queries: Vec<Query> = export_query_strs
            .iter()
            .map(|q| compile_query(&language, q, "export"))
            .collect::<Result<Vec<_>>>()?;

        let import_query = compile_query(
            &language,
            r#"(import_statement source: (string) @source)"#,
            "import",
        )?;
        // ALP-749/750: captures `from '...'` on both `export { X } from` and `export * from`
        let reexport_source_query = compile_query(
            &language,
            r#"(export_statement source: (string) @source)"#,
            "reexport_source",
        )?;
        // ALP-754: simple decorator `@Foo`
        let decorator_query =
            compile_query(&language, "(decorator (identifier) @name)", "decorator")?;
        // ALP-754: call-expression decorator `@Foo(...)`
        let call_decorator_query = compile_query(
            &language,
            "(decorator (call_expression function: (identifier) @name))",
            "call_decorator",
        )?;
        // ALP-768: find class declarations for public method extraction
        let class_query = compile_query(
            &language,
            "(class_declaration name: (type_identifier) @class_name) @class",
            "class",
        )?;
        // ALP-881: named import specifiers — captures original name and source path.
        // Handles `import { foo } from '...'` and `import { foo as bar } from '...'`.
        // `name` = original exported name; ignore `alias` (local binding).
        let named_import_query = compile_query(
            &language,
            r#"(import_statement
                 (import_clause
                   (named_imports
                     (import_specifier name: (identifier) @original_name)))
                 source: (string) @source)"#,
            "named_import",
        )?;
        // ALP-881: namespace imports — `import * as X from '...'`
        let namespace_import_query = compile_query(
            &language,
            r#"(import_statement
                 (import_clause (namespace_import))
                 source: (string) @source)"#,
            "namespace_import",
        )?;
        // ALP-881: named re-export specifiers — `export { foo } from '...'` and `export { foo as bar } from '...'`
        let reexport_named_query = compile_query(
            &language,
            r#"(export_statement
                 (export_clause
                   (export_specifier name: (identifier) @original_name))
                 source: (string) @source)"#,
            "reexport_named",
        )?;

        Ok(Self {
            parser,
            export_queries,
            import_query,
            reexport_source_query,
            decorator_query,
            call_decorator_query,
            class_query,
            named_import_query,
            namespace_import_query,
            reexport_named_query,
            is_tsx,
            tsconfig_cache: HashMap::new(),
        })
    }

    /// Extract exports. The first return value is all exports; the second is the set of names
    /// that come from `function_declaration` nodes (ALP-863: used for function_index).
    fn extract_exports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<ExportEntry>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut seen: HashSet<String> = HashSet::new();
        let mut exports = Vec::new();
        let mut function_names: Vec<String> = Vec::new();

        for (qi, query) in self.export_queries.iter().enumerate() {
            for entry in collect_matches_with_lines(query, root_node, source_bytes) {
                if seen.insert(entry.name.clone()) {
                    // Query index 0 matches `export function foo()` — confirmed function decls.
                    if qi == 0 {
                        function_names.push(entry.name.clone());
                    }
                    exports.push(entry);
                }
            }
        }

        exports.sort_by_key(|e| e.start_line);
        (exports, function_names)
    }

    /// ALP-754: extract unique decorator names from the file.
    fn extract_decorators(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut decorators = Vec::new();

        for query in [&self.decorator_query, &self.call_decorator_query] {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes)
                        && seen.insert(text.to_string())
                    {
                        decorators.push(text.to_string());
                    }
                }
            }
        }

        decorators.sort();
        decorators
    }
}

impl Parser for TypeScriptParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        self.parse_with_aliases(source, &HashMap::new())
    }

    /// ALP-794: override parse_file() to load tsconfig path aliases from the
    /// file's directory tree and use them to classify alias imports as local deps.
    /// ALP-925: result is cached per directory so the filesystem walk happens
    /// at most once per unique parent directory per parser instance.
    fn parse_file(&mut self, source: &str, file_path: &Path) -> Result<ParseResult> {
        let dir = file_path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
        // Clone releases the borrow of tsconfig_cache before parse_with_aliases
        // borrows self again. Path alias maps are small (≤ tens of entries).
        let aliases = self
            .tsconfig_cache
            .entry(dir)
            .or_insert_with(|| load_tsconfig_paths(file_path))
            .clone();
        self.parse_with_aliases(source, &aliases)
    }

    fn language_id(&self) -> &'static str {
        if self.is_tsx { "tsx" } else { "typescript" }
    }

    fn extensions(&self) -> &'static [&'static str] {
        if self.is_tsx {
            &["tsx", "jsx"]
        } else {
            &["ts", "js"]
        }
    }
}

impl TypeScriptParser {
    fn parse_with_aliases(
        &mut self,
        source: &str,
        aliases: &HashMap<String, Vec<String>>,
    ) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source code"))?;

        let root_node = tree.root_node();

        let (mut exports, function_names) = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node, aliases);
        let (dependencies, dependency_kinds) =
            self.extract_dependencies(source, root_node, aliases);
        let (named_imports, namespace_imports) = self.extract_named_imports(source, root_node);
        let loc = source.lines().count();

        // ALP-768: extract public methods from exported classes
        let exported_classes: HashSet<String> = exports
            .iter()
            .filter(|e| e.parent_class.is_none())
            .map(|e| e.name.clone())
            .collect();
        let methods = self.extract_class_methods(source, root_node, &exported_classes);
        exports.extend(methods);

        // ALP-922: extract depth-1 nested function declarations and prologue vars
        let nested = Self::extract_nested_symbols(source, root_node);
        exports.extend(nested);

        exports.sort_by_key(|e| e.start_line);

        let decorators = self.extract_decorators(source, root_node);
        let custom_fields = if decorators.is_empty() && function_names.is_empty() {
            None
        } else {
            let mut fields = HashMap::new();
            if !decorators.is_empty() {
                fields.insert(
                    "decorators".to_string(),
                    serde_json::Value::Array(
                        decorators
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            // ALP-863: store confirmed function declaration names so the manifest can build
            // function_index for call-site precision in fmm_glossary.
            if !function_names.is_empty() {
                fields.insert(
                    "function_names".to_string(),
                    serde_json::Value::Array(
                        function_names
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            Some(fields)
        };

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                dependency_kinds,
                loc,
                named_imports,
                namespace_imports,
            },
            custom_fields,
        })
    }
}

pub(crate) const TS_DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "typescript",
        extensions: &["ts", "js"],
        reexport_filenames: &["index.ts", "index.js", "index.tsx", "index.jsx"],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &[
                ".spec.ts",
                ".test.ts",
                ".spec.js",
                ".test.js",
                ".spec.tsx",
                ".test.tsx",
            ],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };

pub(crate) const TSX_DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "tsx",
        extensions: &["tsx", "jsx"],
        reexport_filenames: &["index.ts", "index.js", "index.tsx", "index.jsx"],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &[
                ".spec.ts",
                ".test.ts",
                ".spec.js",
                ".test.js",
                ".spec.tsx",
                ".test.tsx",
            ],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };
