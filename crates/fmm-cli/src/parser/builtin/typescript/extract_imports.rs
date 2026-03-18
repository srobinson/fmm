use std::collections::{HashMap, HashSet};

use streaming_iterator::StreamingIterator;
use tree_sitter::QueryCursor;

use super::tsconfig::resolve_alias;
use super::TypeScriptParser;

impl TypeScriptParser {
    pub(super) fn extract_imports(
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

    pub(super) fn extract_dependencies(
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
