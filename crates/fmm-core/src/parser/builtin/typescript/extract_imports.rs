use std::collections::{HashMap, HashSet};

use streaming_iterator::StreamingIterator;
use tree_sitter::QueryCursor;

use crate::identity::EdgeKind;

use super::TypeScriptParser;
use super::tsconfig::resolve_alias;

impl TypeScriptParser {
    pub(super) fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
        aliases: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();

        for query in [&self.import_query, &self.reexport_source_query] {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);

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
        }

        let mut imports: Vec<String> = seen.into_iter().collect();
        imports.sort();
        imports
    }

    pub(super) fn extract_dependencies(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
        aliases: &HashMap<String, Vec<String>>,
    ) -> (Vec<String>, HashMap<String, EdgeKind>) {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut kinds = HashMap::new();

        // Regular import statements
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let cleaned = text.trim_matches('\'').trim_matches('"').to_string();
                    let kind = import_statement_kind(capture.node);
                    if cleaned.starts_with('.') || cleaned.starts_with('/') {
                        insert_dependency(&mut seen, &mut kinds, cleaned, kind);
                    } else if !aliases.is_empty()
                        && let Some(resolved) = resolve_alias(&cleaned, aliases)
                    {
                        // ALP-794: path alias — resolve to physical path and treat as local dep.
                        insert_dependency(&mut seen, &mut kinds, resolved, kind);
                    } else {
                        insert_dependency_kind(&mut kinds, cleaned, kind);
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
                    let kind = export_statement_kind(capture.node);
                    if cleaned.starts_with('.') || cleaned.starts_with('/') {
                        insert_dependency(&mut seen, &mut kinds, cleaned, kind);
                    } else if !aliases.is_empty()
                        && let Some(resolved) = resolve_alias(&cleaned, aliases)
                    {
                        insert_dependency(&mut seen, &mut kinds, resolved, kind);
                    } else {
                        insert_dependency_kind(&mut kinds, cleaned, kind);
                    }
                }
            }
        }

        let mut dependencies: Vec<String> = seen.into_iter().collect();
        dependencies.sort();
        (dependencies, kinds)
    }

    /// ALP-881: extract named imports per source module and namespace import paths.
    ///
    /// Returns `(named_imports, namespace_imports)`:
    /// - `named_imports`: map of source path to original exported names (alias-resolved).
    ///   Includes both `import { foo } from '...'` and `export { foo } from '...'`.
    /// - `namespace_imports`: source paths from `import * as X from '...'` and `export * from '...'`.
    pub(super) fn extract_named_imports(
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
}

fn insert_dependency(
    seen: &mut HashSet<String>,
    kinds: &mut HashMap<String, EdgeKind>,
    dependency: String,
    kind: EdgeKind,
) {
    seen.insert(dependency.clone());
    insert_dependency_kind(kinds, dependency, kind);
}

fn insert_dependency_kind(
    kinds: &mut HashMap<String, EdgeKind>,
    dependency: String,
    kind: EdgeKind,
) {
    kinds
        .entry(dependency)
        .and_modify(|existing| {
            if kind == EdgeKind::Runtime {
                *existing = EdgeKind::Runtime;
            }
        })
        .or_insert(kind);
}

/// Classify an `import_statement` source by walking the AST. The string form
/// is intentionally not consulted because identifier prefixes like
/// `typescriptCompiler` would alias the `type` keyword.
fn import_statement_kind(source_node: tree_sitter::Node) -> EdgeKind {
    let Some(statement) = ancestor_kind(source_node, "import_statement") else {
        return EdgeKind::Runtime;
    };
    classify_type_import_or_export(
        statement,
        "import_clause",
        &["identifier", "namespace_import"],
        "named_imports",
        "import_specifier",
    )
}

/// Classify an `export_statement` re-export source. Mirrors the import
/// classifier so `export type { X } from './y'` and `export { type X } from './y'`
/// are preserved as type-only edges.
fn export_statement_kind(source_node: tree_sitter::Node) -> EdgeKind {
    let Some(statement) = ancestor_kind(source_node, "export_statement") else {
        return EdgeKind::Runtime;
    };
    classify_type_import_or_export(
        statement,
        "export_clause",
        &[],
        "export_clause",
        "export_specifier",
    )
}

/// Shared classifier for `import_statement` and `export_statement` re-exports.
///
/// A statement is type-only when either:
/// - the statement has a direct `type` keyword child (e.g. `import type ...`,
///   `export type ...`), or
/// - the only binding is a named clause and every specifier within that
///   clause has its own `type` keyword child.
///
/// Any default identifier or namespace binding sibling of the named clause
/// forces a runtime classification because those bindings are runtime values.
fn classify_type_import_or_export(
    statement: tree_sitter::Node,
    clause_kind: &str,
    runtime_sibling_kinds: &[&str],
    named_clause_kind: &str,
    specifier_kind: &str,
) -> EdgeKind {
    let mut cursor = statement.walk();
    let mut clause: Option<tree_sitter::Node> = None;
    for child in statement.children(&mut cursor) {
        match child.kind() {
            "type" => return EdgeKind::TypeOnly,
            kind if kind == clause_kind => clause = Some(child),
            _ => {}
        }
    }

    let Some(clause) = clause else {
        // Side-effect import (`import './foo'`) or wildcard re-export (`export * from`)
        // has no named clause; it always produces a runtime edge.
        return EdgeKind::Runtime;
    };

    let mut cursor = clause.walk();
    let mut named: Option<tree_sitter::Node> = None;
    for child in clause.children(&mut cursor) {
        let kind = child.kind();
        if runtime_sibling_kinds.contains(&kind) {
            return EdgeKind::Runtime;
        }
        if kind == named_clause_kind {
            named = Some(child);
        }
    }

    // For exports the clause itself is the named clause; for imports the
    // named_imports node is a child of import_clause.
    let named = named.unwrap_or(clause);

    let mut cursor = named.walk();
    let mut any_specifier = false;
    for child in named.children(&mut cursor) {
        if child.kind() != specifier_kind {
            continue;
        }
        any_specifier = true;
        let mut spec_cursor = child.walk();
        let has_type = child
            .children(&mut spec_cursor)
            .any(|grandchild| grandchild.kind() == "type");
        if !has_type {
            return EdgeKind::Runtime;
        }
    }

    if any_specifier {
        EdgeKind::TypeOnly
    } else {
        EdgeKind::Runtime
    }
}

fn ancestor_kind<'tree>(
    mut node: tree_sitter::Node<'tree>,
    kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}
