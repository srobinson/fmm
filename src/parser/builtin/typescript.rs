use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

use super::query_helpers::{collect_matches_with_lines, compile_query, make_parser};

/// Walk up from `file_path` looking for `tsconfig.json`. When found, extract
/// `compilerOptions.paths` as a map of alias pattern → list of target templates.
/// Follows `extends` one level deep to pick up a base config's `paths`.
/// Returns an empty map when no tsconfig is found or no paths are configured.
fn load_tsconfig_paths(file_path: &Path) -> HashMap<String, Vec<String>> {
    let mut dir = file_path.parent();
    while let Some(d) = dir {
        let tsconfig = d.join("tsconfig.json");
        if tsconfig.exists() {
            return read_tsconfig_paths(&tsconfig);
        }
        dir = d.parent();
    }
    HashMap::new()
}

/// Read `compilerOptions.paths` from a tsconfig file. Follows `extends` one
/// level deep so that a base config's paths are included.
fn read_tsconfig_paths(tsconfig: &Path) -> HashMap<String, Vec<String>> {
    let Ok(content) = std::fs::read_to_string(tsconfig) else {
        return HashMap::new();
    };
    // tsconfig.json may contain comments — use serde_json with a best-effort
    // approach by stripping single-line comments first.
    let stripped = strip_json_comments(&content);
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&stripped) else {
        return HashMap::new();
    };

    let mut paths: HashMap<String, Vec<String>> = HashMap::new();

    // Follow `extends` one level deep to pick up base config paths.
    if let Some(extends) = json.get("extends").and_then(|v| v.as_str()) {
        let base_path = tsconfig.parent().unwrap_or(Path::new(".")).join(extends);
        let base = read_tsconfig_paths(&base_path);
        paths.extend(base);
    }

    // Own paths override base.
    if let Some(own_paths) = json
        .get("compilerOptions")
        .and_then(|o| o.get("paths"))
        .and_then(|p| p.as_object())
    {
        for (alias, targets) in own_paths {
            if let Some(target_arr) = targets.as_array() {
                let target_strings: Vec<String> = target_arr
                    .iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect();
                if !target_strings.is_empty() {
                    paths.insert(alias.clone(), target_strings);
                }
            }
        }
    }

    paths
}

/// Strip single-line `//` comments from JSON-like content so that tsconfig
/// files with comments can be parsed by serde_json.
fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                // Escaped character — include next char verbatim.
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            // Single-line comment — skip to end of line.
            for ch in chars.by_ref() {
                if ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Attempt to resolve a TypeScript path alias import to its physical path.
///
/// Given an import string like `@/utils/helper` and an aliases map like
/// `{"@/*": ["src/*"]}`, returns `Some("src/utils/helper")`. Returns `None`
/// when no alias matches.
fn resolve_alias(import: &str, aliases: &HashMap<String, Vec<String>>) -> Option<String> {
    for (pattern, targets) in aliases {
        if let Some(resolved) = match_alias(import, pattern, targets) {
            return Some(resolved);
        }
    }
    None
}

/// Try a single alias pattern against the import. Patterns ending with `*`
/// act as prefix matches; exact patterns must match the full import string.
fn match_alias(import: &str, pattern: &str, targets: &[String]) -> Option<String> {
    let target = targets.first()?; // Use first target mapping.
    if let Some(prefix) = pattern.strip_suffix('*') {
        // Wildcard: `@/*` matches `@/foo/bar`, captures `foo/bar`.
        if let Some(rest) = import.strip_prefix(prefix) {
            if let Some(target_prefix) = target.strip_suffix('*') {
                return Some(format!("{}{}", target_prefix, rest));
            }
            // Target has no wildcard — map the whole import to the target.
            return Some(target.clone());
        }
    } else if import == pattern {
        // Exact match — map to first target (strip trailing `/*` if present).
        let mapped = target.strip_suffix("/*").unwrap_or(target).to_string();
        return Some(mapped);
    }
    None
}

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
    /// Parser for `.ts` and `.js` files — uses `LANGUAGE_TYPESCRIPT`.
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

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
        aliases: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    // Relative paths go to dependencies; alias matches go to dependencies.
                    // Everything else (external packages) stays here as imports.
                    if !cleaned.starts_with('.')
                        && !cleaned.starts_with('/')
                        && (aliases.is_empty() || resolve_alias(&cleaned, aliases).is_none())
                    {
                        seen.insert(cleaned);
                    }
                }
            }
        }

        let mut imports: Vec<String> = seen.into_iter().collect();
        imports.sort();
        imports
    }

    fn extract_dependencies(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
        aliases: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        // Regular import statements
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if cleaned.starts_with('.') || cleaned.starts_with('/') {
                        seen.insert(cleaned);
                    } else if !aliases.is_empty() {
                        // ALP-794: path alias — resolve to physical path and treat as local dep.
                        if let Some(resolved) = resolve_alias(&cleaned, aliases) {
                            seen.insert(resolved);
                        }
                    }
                }
            }
        }

        // ALP-749/750: re-export sources — `export { X } from './y'` and `export * from './y'`
        let mut cursor2 = QueryCursor::new();
        let mut iter2 = cursor2.matches(&self.reexport_source_query, root_node, source_bytes);
        while let Some(m) = iter2.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    if cleaned.starts_with('.') || cleaned.starts_with('/') {
                        seen.insert(cleaned);
                    } else if !aliases.is_empty() {
                        if let Some(resolved) = resolve_alias(&cleaned, aliases) {
                            seen.insert(resolved);
                        }
                    }
                }
            }
        }

        let mut dependencies: Vec<String> = seen.into_iter().collect();
        dependencies.sort();
        dependencies
    }

    /// ALP-768: extract public methods from exported classes.
    /// Returns `ExportEntry` items with `parent_class` set to the class name.
    fn extract_class_methods(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
        exported_class_names: &HashSet<String>,
    ) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut entries = Vec::new();

        let class_name_idx = self
            .class_query
            .capture_index_for_name("class_name")
            .unwrap_or(0);
        let class_idx = self
            .class_query
            .capture_index_for_name("class")
            .unwrap_or(1);

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.class_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            let mut class_node: Option<tree_sitter::Node> = None;
            let mut class_name: Option<String> = None;

            for cap in m.captures {
                if cap.index == class_name_idx {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        class_name = Some(text.to_string());
                    }
                } else if cap.index == class_idx {
                    class_node = Some(cap.node);
                }
            }

            let (class_node, class_name) = match (class_node, class_name) {
                (Some(n), Some(name)) => (n, name),
                _ => continue,
            };

            if !exported_class_names.contains(&class_name) {
                continue;
            }

            let body = match class_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            for i in 0..body.child_count() {
                if let Some(child) = body.child(i as u32) {
                    if child.kind() == "method_definition" {
                        if let Some(entry) =
                            Self::extract_method_entry(&class_name, child, source_bytes)
                        {
                            entries.push(entry);
                        }
                    }
                }
            }
        }

        entries
    }

    /// Extract a single method_definition node as an ExportEntry.
    /// Returns None for private or protected methods.
    fn extract_method_entry(
        class_name: &str,
        method_node: tree_sitter::Node,
        source_bytes: &[u8],
    ) -> Option<ExportEntry> {
        // Check accessibility_modifier — skip private and protected
        for i in 0..method_node.child_count() {
            if let Some(child) = method_node.child(i as u32) {
                if child.kind() == "accessibility_modifier" {
                    let text = child.utf8_text(source_bytes).unwrap_or("");
                    if text == "private" || text == "protected" {
                        return None;
                    }
                }
            }
        }

        // Get method name from the "name" field
        let name_node = method_node.child_by_field_name("name")?;
        let method_name = name_node.utf8_text(source_bytes).ok()?.to_string();

        // Skip empty names, computed property names ([Symbol.iterator]), and private fields (#foo)
        if method_name.is_empty() || method_name.starts_with('[') || method_name.starts_with('#') {
            return None;
        }

        Some(ExportEntry::method(
            method_name,
            method_node.start_position().row + 1,
            method_node.end_position().row + 1,
            class_name.to_string(),
        ))
    }

    /// ALP-881: extract named imports per source module and namespace import paths.
    ///
    /// Returns `(named_imports, namespace_imports)`:
    /// - `named_imports`: map of source path → original exported names (alias-resolved).
    ///   Includes both `import { foo } from '...'` and `export { foo } from '...'`.
    /// - `namespace_imports`: source paths from `import * as X from '...'` and `export * from '...'`.
    fn extract_named_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (HashMap<String, Vec<String>>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut named: HashMap<String, Vec<String>> = HashMap::new();
        let mut namespace: Vec<String> = Vec::new();
        let mut namespace_seen: HashSet<String> = HashSet::new();

        let source_idx = self.named_import_query.capture_index_for_name("source");
        let name_idx = self
            .named_import_query
            .capture_index_for_name("original_name");

        // Named import statements: `import { foo, foo as bar } from './mod'`
        if let (Some(src_idx), Some(nm_idx)) = (source_idx, name_idx) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&self.named_import_query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                let mut source_path: Option<String> = None;
                let mut orig_name: Option<String> = None;
                for cap in m.captures {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        if cap.index == src_idx {
                            source_path =
                                Some(text.trim_matches('\'').trim_matches('"').to_string());
                        } else if cap.index == nm_idx {
                            orig_name = Some(text.to_string());
                        }
                    }
                }
                if let (Some(path), Some(name)) = (source_path, orig_name) {
                    named.entry(path).or_default().push(name);
                }
            }
        }

        // Named re-export specifiers: `export { foo } from './mod'`
        let re_source_idx = self.reexport_named_query.capture_index_for_name("source");
        let re_name_idx = self
            .reexport_named_query
            .capture_index_for_name("original_name");
        if let (Some(src_idx), Some(nm_idx)) = (re_source_idx, re_name_idx) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&self.reexport_named_query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                let mut source_path: Option<String> = None;
                let mut orig_name: Option<String> = None;
                for cap in m.captures {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        if cap.index == src_idx {
                            source_path =
                                Some(text.trim_matches('\'').trim_matches('"').to_string());
                        } else if cap.index == nm_idx {
                            orig_name = Some(text.to_string());
                        }
                    }
                }
                if let (Some(path), Some(name)) = (source_path, orig_name) {
                    named.entry(path).or_default().push(name);
                }
            }
        }

        // Namespace imports: `import * as X from './mod'`
        {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&self.namespace_import_query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for cap in m.captures {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        let path = text.trim_matches('\'').trim_matches('"').to_string();
                        if namespace_seen.insert(path.clone()) {
                            namespace.push(path);
                        }
                    }
                }
            }
        }

        // Wildcard re-exports: `export * from './mod'` and `export * as ns from './mod'`.
        // Detected as: re-export sources (reexport_source_query) minus named re-export sources.
        // Any source that appears in reexport_source_query but has no named specifiers is a wildcard.
        {
            let named_reexport_sources: HashSet<&String> = named.keys().collect();
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&self.reexport_source_query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for cap in m.captures {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        let path = text.trim_matches('\'').trim_matches('"').to_string();
                        if !named_reexport_sources.contains(&path)
                            && namespace_seen.insert(path.clone())
                        {
                            namespace.push(path);
                        }
                    }
                }
            }
        }

        // Deduplicate names within each source entry and sort for stable output.
        for names in named.values_mut() {
            names.sort();
            names.dedup();
        }
        namespace.sort();

        (named, namespace)
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
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        if seen.insert(text.to_string()) {
                            decorators.push(text.to_string());
                        }
                    }
                }
            }
        }

        decorators.sort();
        decorators
    }
}

impl TypeScriptParser {
    /// ALP-922: Extract depth-1 nested function declarations and prologue variables
    /// from all top-level function bodies. Only processes JS/TS function_declaration nodes
    /// (exported or bare). Arrow functions assigned to variables are skipped — they have
    /// no named nested declarations in practice.
    fn extract_nested_symbols(
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Vec<crate::parser::ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut entries = Vec::new();

        for i in 0..root_node.child_count() {
            let child = match root_node.child(i as u32) {
                Some(c) => c,
                None => continue,
            };

            let fn_node = match child.kind() {
                "function_declaration" => Some(child),
                "export_statement" => {
                    // exported function_declaration is typically the second child
                    let mut found = None;
                    for j in 0..child.child_count() {
                        if let Some(c) = child.child(j as u32) {
                            if c.kind() == "function_declaration" {
                                found = Some(c);
                                break;
                            }
                        }
                    }
                    found
                }
                _ => None,
            };

            let fn_node = match fn_node {
                Some(n) => n,
                None => continue,
            };

            let fn_name = match fn_node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
            {
                Some(n) => n.to_string(),
                None => continue,
            };

            let body = match fn_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            let mut first_nested_fn_seen = false;

            for j in 0..body.child_count() {
                let stmt = match body.child(j as u32) {
                    Some(s) => s,
                    None => continue,
                };

                match stmt.kind() {
                    "function_declaration" => {
                        first_nested_fn_seen = true;
                        let nested_name = match stmt
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source_bytes).ok())
                        {
                            Some(n) => n.to_string(),
                            None => continue,
                        };
                        entries.push(crate::parser::ExportEntry::nested_fn(
                            nested_name,
                            stmt.start_position().row + 1,
                            stmt.end_position().row + 1,
                            fn_name.clone(),
                        ));
                    }
                    "lexical_declaration" | "variable_declaration" if !first_nested_fn_seen => {
                        // Prologue: extract individual declarators that are non-trivial
                        for k in 0..stmt.child_count() {
                            let decl = match stmt.child(k as u32) {
                                Some(d) if d.kind() == "variable_declarator" => d,
                                _ => continue,
                            };
                            let var_name = match decl
                                .child_by_field_name("name")
                                .and_then(|n| n.utf8_text(source_bytes).ok())
                            {
                                Some(n) => n.to_string(),
                                None => continue,
                            };
                            if Self::is_non_trivial_declarator(decl) {
                                entries.push(crate::parser::ExportEntry::closure_state(
                                    var_name,
                                    decl.start_position().row + 1,
                                    decl.end_position().row + 1,
                                    fn_name.clone(),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        entries
    }

    /// Return true when a variable_declarator is worth indexing as closure-state:
    /// it has a type annotation, or its value starts with a call expression.
    fn is_non_trivial_declarator(decl: tree_sitter::Node) -> bool {
        // Check for type_annotation child
        for i in 0..decl.child_count() {
            if let Some(child) = decl.child(i as u32) {
                if child.kind() == "type_annotation" {
                    return true;
                }
            }
        }
        // Check value for call_expression (or as_expression wrapping one)
        if let Some(value) = decl.child_by_field_name("value") {
            if value.kind() == "call_expression" {
                return true;
            }
            // Handle `foo() as Type` (as_expression) or `new Foo()` (new_expression)
            if value.kind() == "as_expression" || value.kind() == "new_expression" {
                return true;
            }
            // One level deeper: `(call())` — parenthesized expression
            for i in 0..value.child_count() {
                if let Some(child) = value.child(i as u32) {
                    if child.kind() == "call_expression" || child.kind() == "new_expression" {
                        return true;
                    }
                }
            }
        }
        false
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
        if self.is_tsx {
            "tsx"
        } else {
            "typescript"
        }
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
        let dependencies = self.extract_dependencies(source, root_node, aliases);
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

impl crate::parser::LanguageDescriptor for TypeScriptParser {
    fn language_id(&self) -> &'static str {
        if self.is_tsx {
            "tsx"
        } else {
            "typescript"
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        if self.is_tsx {
            &["tsx", "jsx"]
        } else {
            &["ts", "js"]
        }
    }

    fn reexport_filenames(&self) -> &'static [&'static str] {
        // Both TS and TSX variants share the same index.* re-export convention.
        &["index.ts", "index.js", "index.tsx", "index.jsx"]
    }

    fn test_file_patterns(&self) -> crate::parser::LanguageTestPatterns {
        crate::parser::LanguageTestPatterns {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> ParseResult {
        let mut parser = TypeScriptParser::new().unwrap();
        parser.parse(source).unwrap()
    }

    fn parse_tsx(source: &str) -> ParseResult {
        let mut parser = TypeScriptParser::new_tsx().unwrap();
        parser.parse(source).unwrap()
    }

    // --- Export extraction ---

    #[test]
    fn exports_named_function() {
        let result = parse("export function greet(name: string) { return `Hi ${name}`; }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"greet".to_string()));
    }

    #[test]
    fn exports_arrow_function_via_const() {
        let result = parse("export const add = (a: number, b: number) => a + b;");
        assert!(result.metadata.export_names().contains(&"add".to_string()));
    }

    #[test]
    fn exports_class() {
        let result = parse("export class UserService { constructor() {} }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"UserService".to_string()));
    }

    #[test]
    fn exports_interface() {
        let result = parse("export interface Config { debug: boolean; }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Config".to_string()));
    }

    #[test]
    fn exports_multiple_from_clause() {
        let result = parse("export { foo, bar, baz } from './other';");
        assert!(result.metadata.export_names().contains(&"foo".to_string()));
        assert!(result.metadata.export_names().contains(&"bar".to_string()));
        assert!(result.metadata.export_names().contains(&"baz".to_string()));
    }

    #[test]
    fn exports_const_variable() {
        let result = parse("export const MAX_RETRIES = 3;");
        assert!(result
            .metadata
            .export_names()
            .contains(&"MAX_RETRIES".to_string()));
    }

    #[test]
    fn exports_let_variable() {
        let result = parse("export let counter = 0;");
        assert!(result
            .metadata
            .export_names()
            .contains(&"counter".to_string()));
    }

    #[test]
    fn exports_are_sorted_by_line_and_deduplicated() {
        let source = r#"
export function zebra() {}
export function alpha() {}
export const middle = 1;
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["zebra", "alpha", "middle"]
        );
    }

    // --- ALP-751: Enum exports ---

    #[test]
    fn exports_enum() {
        let result = parse("export enum Direction { Up, Down, Left, Right }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Direction".to_string()));
    }

    #[test]
    fn exports_const_enum() {
        let result = parse("export const enum Status { Active, Inactive }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Status".to_string()));
    }

    #[test]
    fn exports_enum_line_range() {
        let source = "// header\nexport enum Color {\n    Red,\n    Green,\n    Blue,\n}\n";
        let result = parse(source);
        let entry = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Color")
            .unwrap();
        assert_eq!(entry.start_line, 2);
        assert_eq!(entry.end_line, 6);
    }

    // --- ALP-752: Renamed export specifiers ---

    #[test]
    fn exports_aliased_specifier_captures_alias() {
        let result = parse("export { foo as bar } from './mod';");
        assert!(result.metadata.export_names().contains(&"bar".to_string()));
        assert!(!result.metadata.export_names().contains(&"foo".to_string()));
    }

    #[test]
    fn exports_unaliased_specifier_unchanged() {
        let result = parse("export { foo } from './mod';");
        assert!(result.metadata.export_names().contains(&"foo".to_string()));
    }

    #[test]
    fn exports_mixed_aliased_and_unaliased() {
        let result = parse("export { a as b, c } from './mod';");
        assert!(result.metadata.export_names().contains(&"b".to_string()));
        assert!(result.metadata.export_names().contains(&"c".to_string()));
        assert!(!result.metadata.export_names().contains(&"a".to_string()));
    }

    #[test]
    fn exports_aliased_specifier_with_dep_capture() {
        let result = parse("export { foo as bar } from './mod';");
        assert!(result.metadata.dependencies.contains(&"./mod".to_string()));
    }

    // --- ALP-755: export * as namespace ---

    #[test]
    fn exports_namespace_star_reexport() {
        let result = parse("export * as utils from './utils';");
        assert!(result
            .metadata
            .export_names()
            .contains(&"utils".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils".to_string()));
    }

    // --- ALP-756: export namespace / module ---

    #[test]
    fn exports_namespace_declaration() {
        let result = parse("export namespace Validation { export function isEmail(s: string): boolean { return true; } }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Validation".to_string()));
    }

    #[test]
    fn exports_module_declaration() {
        let result = parse("export module Shapes { export class Circle {} }");
        assert!(result
            .metadata
            .export_names()
            .contains(&"Shapes".to_string()));
    }

    // --- Import extraction ---

    #[test]
    fn imports_external_package() {
        let result = parse("import { useState } from 'react';");
        assert!(result.metadata.imports.contains(&"react".to_string()));
    }

    #[test]
    fn imports_scoped_package() {
        let result = parse("import express from '@types/express';");
        assert!(result
            .metadata
            .imports
            .contains(&"@types/express".to_string()));
    }

    #[test]
    fn imports_excludes_relative_paths() {
        let source = r#"
import { helper } from './utils';
import { config } from '../config';
import React from 'react';
"#;
        let result = parse(source);
        assert_eq!(result.metadata.imports, vec!["react"]);
    }

    // --- Dependency extraction ---

    #[test]
    fn dependencies_captures_relative_imports() {
        let source = r#"
import { foo } from './foo';
import { bar } from '../lib/bar';
import { baz } from '/absolute/baz';
import React from 'react';
"#;
        let result = parse(source);
        assert!(result.metadata.dependencies.contains(&"./foo".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"../lib/bar".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"/absolute/baz".to_string()));
        assert!(!result.metadata.dependencies.contains(&"react".to_string()));
    }

    #[test]
    fn dependencies_excludes_external_packages() {
        let result = parse("import express from 'express'; import cors from 'cors';");
        assert!(result.metadata.dependencies.is_empty());
    }

    // --- ALP-749: Barrel re-export dependency capture ---

    #[test]
    fn barrel_reexport_file() {
        let source = r#"
export { UserService } from './user.service';
export { AuthService } from './auth.service';
export { Logger } from './logger';
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["UserService", "AuthService", "Logger"]
        );
        // ALP-749: re-export sources must appear in dependencies
        assert!(result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./logger".to_string()));
    }

    #[test]
    fn barrel_reexport_mixed_import_and_export_from() {
        let source = r#"
import { Pool } from './db/pool';
export { UserService } from './user.service';
export { AuthService } from './auth.service';
"#;
        let result = parse(source);
        assert!(result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string()));
    }

    #[test]
    fn reexport_external_package_not_in_dependencies() {
        let result = parse("export { foo } from '@scope/pkg';");
        assert!(!result
            .metadata
            .dependencies
            .contains(&"@scope/pkg".to_string()));
    }

    // --- ALP-750: export * from star re-exports ---

    #[test]
    fn star_reexport_adds_dependency_not_export_name() {
        let result = parse("export * from './utils';");
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils".to_string()));
        assert!(!result.metadata.export_names().contains(&"*".to_string()));
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn star_reexport_external_not_in_dependencies() {
        let result = parse("export * from 'some-package';");
        assert!(result.metadata.dependencies.is_empty());
    }

    // --- ALP-753: TSX parser ---

    #[test]
    fn tsx_jsx_parsed_with_tsx_grammar() {
        let source = r#"
export function Button({ label }: { label: string }) {
    return <button>{label}</button>;
}
"#;
        let result = parse_tsx(source);
        assert!(result
            .metadata
            .export_names()
            .contains(&"Button".to_string()));
    }

    #[test]
    fn tsx_jsx_arrow_component() {
        let source = r#"
export const Card = ({ title }: { title: string }) => (
    <div className="card">
        <h2>{title}</h2>
    </div>
);
"#;
        let result = parse_tsx(source);
        assert!(result.metadata.export_names().contains(&"Card".to_string()));
    }

    #[test]
    fn ts_parser_language_id_and_extensions() {
        let parser = TypeScriptParser::new().unwrap();
        assert_eq!(parser.language_id(), "typescript");
        assert_eq!(parser.extensions(), &["ts", "js"]);
    }

    #[test]
    fn tsx_parser_language_id_and_extensions() {
        let parser = TypeScriptParser::new_tsx().unwrap();
        assert_eq!(parser.language_id(), "tsx");
        assert_eq!(parser.extensions(), &["tsx", "jsx"]);
    }

    // --- ALP-754: Decorator extraction ---

    #[test]
    fn decorator_simple_captured() {
        let source = r#"
@Component
export class AppComponent {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Component"));
    }

    #[test]
    fn decorator_call_expression_captured() {
        let source = r#"
@Injectable()
export class UserService {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Injectable"));
    }

    #[test]
    fn decorator_multiple_unique() {
        let source = r#"
@Controller('/users')
export class UserController {}

@Injectable()
export class AuthService {}
"#;
        let result = parse(source);
        let fields = result.custom_fields.expect("should have custom_fields");
        let decorators: Vec<&str> = fields["decorators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(decorators.contains(&"Controller"));
        assert!(decorators.contains(&"Injectable"));
    }

    #[test]
    fn no_decorators_custom_fields_none() {
        let result = parse("export class Plain {}");
        assert!(result.custom_fields.is_none());
    }

    // --- Edge cases ---

    #[test]
    fn empty_file() {
        let result = parse("");
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
        assert!(result.metadata.dependencies.is_empty());
        assert_eq!(result.metadata.loc, 0);
    }

    #[test]
    fn no_exports_internal_module() {
        let source = "const internal = 42;\nfunction helper() {}\n";
        let result = parse(source);
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn loc_counting() {
        let source = "line1\nline2\nline3\nline4\n";
        let result = parse(source);
        assert_eq!(result.metadata.loc, 4);
    }

    #[test]
    fn loc_single_line_no_trailing_newline() {
        let result = parse("export const x = 1;");
        assert_eq!(result.metadata.loc, 1);
    }

    #[test]
    fn complex_module_with_mixed_exports() {
        let source = r#"
import { Logger } from 'winston';
import { Pool } from './db/pool';
import type { Config } from './config';

export interface DatabaseConfig {
    host: string;
    port: number;
}

export class DatabaseService {
    private pool: Pool;

    constructor(config: DatabaseConfig) {
        this.pool = new Pool(config);
    }

    async query(sql: string): Promise<any[]> {
        return this.pool.query(sql);
    }
}

export function createService(config: DatabaseConfig): DatabaseService {
    return new DatabaseService(config);
}

export const DEFAULT_PORT = 5432;
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec![
                "DatabaseConfig",
                "DatabaseService",
                "createService",
                "DEFAULT_PORT"
            ]
        );
        assert_eq!(result.metadata.imports, vec!["winston"]);
        assert!(result
            .metadata
            .dependencies
            .contains(&"./db/pool".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./config".to_string()));
        assert!(result.metadata.loc > 20);
    }

    // --- ALP-768: Public class method extraction ---

    #[test]
    fn class_public_method_indexed() {
        let source = "export class Foo {\n  public bar(): void {}\n}\n";
        let result = parse(source);
        let method = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "bar")
            .unwrap();
        assert_eq!(method.parent_class.as_deref(), Some("Foo"));
        assert_eq!(method.start_line, 2);
        // export_names() excludes methods
        assert!(!result.metadata.export_names().contains(&"bar".to_string()));
        assert!(result.metadata.export_names().contains(&"Foo".to_string()));
    }

    #[test]
    fn class_private_method_not_indexed() {
        let source = "export class Foo {\n  private baz(): void {}\n}\n";
        let result = parse(source);
        assert!(!result.metadata.exports.iter().any(|e| e.name == "baz"));
    }

    #[test]
    fn class_protected_method_not_indexed() {
        let source = "export class Foo {\n  protected qux(): void {}\n}\n";
        let result = parse(source);
        assert!(!result.metadata.exports.iter().any(|e| e.name == "qux"));
    }

    #[test]
    fn class_constructor_indexed() {
        let source = "export class Foo {\n  constructor(x: number) {}\n}\n";
        let result = parse(source);
        let ctor = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "constructor");
        assert!(ctor.is_some(), "constructor should be indexed");
        assert_eq!(ctor.unwrap().parent_class.as_deref(), Some("Foo"));
    }

    #[test]
    fn class_no_modifier_is_public() {
        let source = "export class Foo {\n  doThing(): void {}\n}\n";
        let result = parse(source);
        assert!(result.metadata.exports.iter().any(|e| e.name == "doThing"));
    }

    #[test]
    fn non_exported_class_methods_not_indexed() {
        let source = "class Internal {\n  run(): void {}\n}\n";
        let result = parse(source);
        assert!(!result.metadata.exports.iter().any(|e| e.name == "run"));
    }

    #[test]
    fn class_method_line_range_correct() {
        let source = "export class Svc {\n  create() {\n    return 1;\n  }\n  destroy() {}\n}\n";
        let result = parse(source);
        let create = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "create")
            .unwrap();
        assert_eq!(create.start_line, 2);
        assert_eq!(create.end_line, 4);
        let destroy = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "destroy")
            .unwrap();
        assert_eq!(destroy.start_line, 5);
    }

    // --- Default export extraction ---

    #[test]
    fn exports_default_function() {
        let result = parse("export default function App() { return null; }");
        assert_eq!(result.metadata.export_names(), vec!["App"]);
    }

    #[test]
    fn exports_default_class() {
        let result = parse("export default class Router { navigate() {} }");
        assert_eq!(result.metadata.export_names(), vec!["Router"]);
    }

    #[test]
    fn exports_default_identifier() {
        let source = "const Component = () => null;\nexport default Component;";
        let result = parse(source);
        assert_eq!(result.metadata.export_names(), vec!["Component"]);
    }

    #[test]
    fn exports_default_anonymous_arrow_skipped() {
        let result = parse("export default () => {};");
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn exports_default_anonymous_object_skipped() {
        let result = parse("export default { key: 'value' };");
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn exports_default_function_line_range() {
        let source = "// header\nexport default function App() {\n  return null;\n}\n";
        let result = parse(source);
        let app = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "App")
            .unwrap();
        assert_eq!(app.start_line, 2);
        assert_eq!(app.end_line, 4);
    }

    #[test]
    fn exports_default_class_line_range() {
        let source = "// header\nexport default class Router {\n  navigate() {}\n}\n";
        let result = parse(source);
        let router = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Router")
            .unwrap();
        assert_eq!(router.start_line, 2);
        assert_eq!(router.end_line, 4);
    }

    #[test]
    fn exports_default_identifier_line_range() {
        let source = "const Foo = 1;\nexport default Foo;\n";
        let result = parse(source);
        let foo = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Foo")
            .unwrap();
        assert_eq!(foo.start_line, 2);
        assert_eq!(foo.end_line, 2);
    }

    // --- Type alias export extraction ---

    #[test]
    fn exports_type_alias() {
        let result = parse("export type User = { name: string; email: string };");
        assert_eq!(result.metadata.export_names(), vec!["User"]);
    }

    #[test]
    fn exports_type_alias_with_generics() {
        let result = parse("export type Nullable<T> = T | null;");
        assert_eq!(result.metadata.export_names(), vec!["Nullable"]);
    }

    #[test]
    fn exports_type_alias_line_range() {
        let source = "// types\nexport type Config = {\n  debug: boolean;\n  port: number;\n};\n";
        let result = parse(source);
        let cfg = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "Config")
            .unwrap();
        assert_eq!(cfg.start_line, 2);
        assert_eq!(cfg.end_line, 5);
    }

    // --- Mixed default + named + type exports ---

    #[test]
    fn exports_default_with_named_and_types() {
        let source = r#"
export type Props = { label: string };
export const VERSION = "1.0";
export default function App() { return null; }
"#;
        let result = parse(source);
        assert_eq!(
            result.metadata.export_names(),
            vec!["Props", "VERSION", "App"]
        );
    }

    #[test]
    fn exports_default_identifier_with_named() {
        let source = r#"
export const helper = () => {};
const Main = () => {};
export default Main;
"#;
        let result = parse(source);
        assert_eq!(result.metadata.export_names(), vec!["helper", "Main"]);
    }

    // --- ALP-794: tsconfig path alias resolution ---

    fn parse_with_aliases_helper(
        source: &str,
        aliases: HashMap<String, Vec<String>>,
    ) -> ParseResult {
        let mut parser = TypeScriptParser::new().unwrap();
        parser.parse_with_aliases(source, &aliases).unwrap()
    }

    #[test]
    fn alias_wildcard_classified_as_dependency() {
        let mut aliases = HashMap::new();
        aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
        let source = r#"import { helper } from "@/utils/helper";"#;
        let result = parse_with_aliases_helper(source, aliases);
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"src/utils/helper".to_string()),
            "alias import should be a dependency, got: {:?}",
            result.metadata.dependencies
        );
        assert!(
            !result
                .metadata
                .imports
                .contains(&"@/utils/helper".to_string()),
            "alias import must not appear in imports, got: {:?}",
            result.metadata.imports
        );
    }

    #[test]
    fn scoped_package_without_alias_stays_external() {
        let mut aliases = HashMap::new();
        aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
        let source = r#"import { Injectable } from "@nestjs/common";"#;
        let result = parse_with_aliases_helper(source, aliases);
        assert!(
            result
                .metadata
                .imports
                .contains(&"@nestjs/common".to_string()),
            "@nestjs/common must stay in imports, got: {:?}",
            result.metadata.imports
        );
        assert!(
            result.metadata.dependencies.is_empty(),
            "no deps expected, got: {:?}",
            result.metadata.dependencies
        );
    }

    #[test]
    fn no_aliases_falls_back_to_heuristic() {
        // Without tsconfig aliases, @/ imports remain as external (existing behavior).
        let source = r#"import { x } from "@/utils/helper";"#;
        let result = parse(source);
        assert!(
            result
                .metadata
                .imports
                .contains(&"@/utils/helper".to_string()),
            "without aliases, @/ import should stay in imports, got: {:?}",
            result.metadata.imports
        );
    }

    #[test]
    fn alias_tilde_pattern() {
        let mut aliases = HashMap::new();
        aliases.insert("~/*".to_string(), vec!["src/*".to_string()]);
        let source = r#"import { config } from "~/config/app";"#;
        let result = parse_with_aliases_helper(source, aliases);
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"src/config/app".to_string()),
            "tilde alias should resolve to dependency, got: {:?}",
            result.metadata.dependencies
        );
    }

    #[test]
    fn alias_exact_pattern() {
        let mut aliases = HashMap::new();
        aliases.insert("@app".to_string(), vec!["src/app".to_string()]);
        let source = r#"import App from "@app";"#;
        let result = parse_with_aliases_helper(source, aliases);
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"src/app".to_string()),
            "exact alias should resolve, got: {:?}",
            result.metadata.dependencies
        );
    }

    // --- ALP-881: named_imports and namespace_imports ---

    #[test]
    fn named_imports_basic() {
        let result = parse("import { foo, bar } from './mod';");
        let ni = &result.metadata.named_imports;
        assert_eq!(ni.get("./mod").unwrap(), &vec!["bar", "foo"]);
    }

    #[test]
    fn named_imports_aliased_stores_original_name() {
        // `import { foo as bar }` → store `foo`, not `bar`
        let result =
            parse("import { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
        let ni = &result.metadata.named_imports;
        let names = ni.get("./ReactFiberWorkLoop").unwrap();
        assert!(
            names.contains(&"scheduleUpdateOnFiber".to_string()),
            "should store original name"
        );
        assert!(
            !names.contains(&"schedule".to_string()),
            "should not store alias"
        );
    }

    #[test]
    fn named_imports_default_import_not_included() {
        // Default imports do not name a specific export by key
        let result = parse("import React from 'react';");
        assert!(
            result.metadata.named_imports.is_empty(),
            "default imports should not appear in named_imports"
        );
    }

    #[test]
    fn namespace_imports_captured() {
        let result = parse("import * as NS from './module';");
        assert!(result
            .metadata
            .namespace_imports
            .contains(&"./module".to_string()));
        assert!(
            result.metadata.named_imports.is_empty(),
            "namespace import should not populate named_imports"
        );
    }

    #[test]
    fn named_reexports_captured() {
        // `export { foo } from './mod'` — captured in named_imports for the source module
        let result = parse("export { scheduleUpdateOnFiber } from './ReactFiberWorkLoop';");
        let ni = &result.metadata.named_imports;
        let names = ni.get("./ReactFiberWorkLoop").unwrap();
        assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
    }

    #[test]
    fn wildcard_reexport_goes_to_namespace_imports() {
        let result = parse("export * from './utils';");
        assert!(result
            .metadata
            .namespace_imports
            .contains(&"./utils".to_string()));
    }

    #[test]
    fn type_only_import_included_in_named_imports() {
        let result = parse("import type { Foo } from './types';");
        let ni = &result.metadata.named_imports;
        assert!(
            ni.contains_key("./types"),
            "type-only import should be included"
        );
        assert!(ni["./types"].contains(&"Foo".to_string()));
    }

    #[test]
    fn named_imports_multiple_sources() {
        let source = r#"
import { a, b } from './mod-a';
import { c } from './mod-b';
"#;
        let result = parse(source);
        let ni = &result.metadata.named_imports;
        assert_eq!(ni["./mod-a"], vec!["a", "b"]);
        assert_eq!(ni["./mod-b"], vec!["c"]);
    }

    #[test]
    fn named_reexport_aliased_stores_original_name() {
        // `export { foo as bar } from './mod'` → store `foo`
        let result =
            parse("export { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
        let ni = &result.metadata.named_imports;
        let names = ni.get("./ReactFiberWorkLoop").unwrap();
        assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
        assert!(!names.contains(&"schedule".to_string()));
    }

    #[test]
    fn strip_json_comments_basic() {
        let input = r#"{ // a comment
  "key": "value" // inline comment
}"#;
        let stripped = strip_json_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn match_alias_wildcard() {
        let targets = vec!["src/*".to_string()];
        assert_eq!(
            match_alias("@/utils/helper", "@/*", &targets),
            Some("src/utils/helper".to_string())
        );
        assert_eq!(match_alias("@nestjs/common", "@/*", &targets), None);
    }

    // --- ALP-922: Nested symbol extraction ---

    #[test]
    fn nested_fn_extracted_from_exported_function() {
        let source = r#"
export function createTypeChecker(host: any): any {
  var silentNeverType = createIntrinsicType(TypeFlags.Never, "never");
  function getIndexType(type: any): any { return undefined; }
  function getReturnType(sig: any): any { return undefined; }
  return {};
}
"#;
        let result = parse(source);
        let nested: Vec<_> = result
            .metadata
            .exports
            .iter()
            .filter(|e| e.parent_class.as_deref() == Some("createTypeChecker"))
            .collect();
        let names: Vec<&str> = nested.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"getIndexType"),
            "getIndexType missing; names={:?}",
            names
        );
        assert!(
            names.contains(&"getReturnType"),
            "getReturnType missing; names={:?}",
            names
        );
        // silentNeverType is closure-state (call expression initializer)
        assert!(
            names.contains(&"silentNeverType"),
            "silentNeverType missing; names={:?}",
            names
        );
    }

    #[test]
    fn nested_fn_has_correct_kind() {
        let source = r#"
export function outer(): void {
  var state = createState();
  function inner(): void {}
}
"#;
        let result = parse(source);
        let inner_entry = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "inner")
            .expect("inner not found");
        assert_eq!(inner_entry.kind.as_deref(), Some("nested-fn"));
        assert_eq!(inner_entry.parent_class.as_deref(), Some("outer"));

        let state_entry = result
            .metadata
            .exports
            .iter()
            .find(|e| e.name == "state")
            .expect("state (closure-state) not found");
        assert_eq!(state_entry.kind.as_deref(), Some("closure-state"));
    }

    #[test]
    fn trivial_var_not_extracted_as_closure_state() {
        let source = r#"
export function outer(): void {
  let counter = 0;
  var flag = false;
  function inner(): void {}
}
"#;
        let result = parse(source);
        let names: Vec<&str> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        // trivial literals must not appear
        assert!(
            !names.contains(&"counter"),
            "trivial counter should not be extracted"
        );
        assert!(
            !names.contains(&"flag"),
            "trivial flag should not be extracted"
        );
    }

    #[test]
    fn depth2_nested_fn_not_extracted() {
        let source = r#"
export function outer(): void {
  function depth1(): void {
    function depth2(): void {}
  }
}
"#;
        let result = parse(source);
        let names: Vec<&str> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(!names.contains(&"depth2"), "depth2 should not be extracted");
        assert!(names.contains(&"depth1"), "depth1 should be extracted");
    }

    #[test]
    fn prologue_var_after_first_nested_fn_not_extracted() {
        let source = r#"
export function outer(): void {
  var before = createA();
  function inner(): void {}
  var after = createB();
}
"#;
        let result = parse(source);
        let names: Vec<&str> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(
            names.contains(&"before"),
            "before (prologue) should be extracted"
        );
        assert!(
            !names.contains(&"after"),
            "after (post-first-fn) should not be extracted"
        );
    }

    #[test]
    fn nested_symbols_in_non_exported_function() {
        let source = r#"
function internalHelper(): void {
  var state = createState();
  function processItem(item: any): void {}
}
"#;
        let result = parse(source);
        let names: Vec<&str> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(
            names.contains(&"processItem"),
            "processItem should be extracted"
        );
        assert!(
            names.contains(&"state"),
            "state closure-state should be extracted"
        );
    }
}
