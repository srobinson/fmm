use super::RustParser;
use crate::parser::ExportEntry;
use crate::parser::builtin::query_helpers::extract_field_text;
use std::collections::HashSet;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

impl RustParser {
    pub(super) fn extract_exports(
        &self,
        source: &str,
        root_node: Node,
        binary_crate: bool,
    ) -> Vec<ExportEntry> {
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        let queries = if binary_crate {
            &self.all_item_queries
        } else {
            &self.export_queries
        };

        for query in queries {
            let capture_names = query.capture_names();
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                if !binary_crate {
                    let vis_capture = m.captures.iter().find(|c| {
                        let idx = c.index as usize;
                        idx < capture_names.len() && capture_names[idx] == "vis"
                    });
                    if let Some(vis) = vis_capture
                        && let Ok(vis_text) = vis.node.utf8_text(source_bytes)
                        && vis_text != "pub"
                    {
                        continue;
                    }
                }

                let name_capture = m.captures.iter().find(|c| {
                    let idx = c.index as usize;
                    idx < capture_names.len() && capture_names[idx] == "name"
                });

                if let Some(name) = name_capture
                    && let Ok(text) = name.node.utf8_text(source_bytes)
                {
                    let name_str = text.to_string();
                    if seen.insert(name_str.clone()) {
                        let decl = name.node.parent().unwrap_or(name.node);
                        exports.push(ExportEntry::new(
                            name_str,
                            decl.start_position().row + 1,
                            decl.end_position().row + 1,
                        ));
                    }
                }
            }
        }

        // Collect pub use re-exports (always, regardless of binary_crate)
        let pub_use_exports = self.extract_pub_use_names(source, root_node);
        for entry in pub_use_exports {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }

        // Collect #[macro_export] declarative macros and proc-macro functions
        let macro_exports = self.extract_macro_exports(source, root_node);
        for entry in macro_exports {
            if seen.insert(entry.name.clone()) {
                exports.push(entry);
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports.dedup_by(|a, b| a.name == b.name && a.parent_class == b.parent_class);
        exports
    }

    /// ALP-770: resolve the self-type name from an impl_item's `type` field.
    /// Handles both plain `type_identifier` and `generic_type` (impl Foo<T>).
    fn impl_type_name(type_node: Node, source_bytes: &[u8]) -> Option<String> {
        match type_node.kind() {
            "type_identifier" => type_node
                .utf8_text(source_bytes)
                .ok()
                .map(|s| s.to_string()),
            "generic_type" => type_node
                .child_by_field_name("type")
                .and_then(|n| Self::impl_type_name(n, source_bytes)),
            _ => None,
        }
    }

    /// ALP-770: extract `pub fn` methods from impl blocks of exported types.
    /// Both `impl Foo {}` and `impl Trait for Foo {}` are covered.
    /// Returns `ExportEntry` items with `parent_class` set to the type name.
    pub(super) fn extract_impl_methods(
        &self,
        source: &str,
        root_node: Node,
        exported_type_names: &HashSet<String>,
    ) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut entries = Vec::new();

        let impl_idx = self.impl_query.capture_index_for_name("impl").unwrap_or(0);

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.impl_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            let impl_node = match m.captures.iter().find(|c| c.index == impl_idx) {
                Some(cap) => cap.node,
                None => continue,
            };

            let type_name = match impl_node
                .child_by_field_name("type")
                .and_then(|n| Self::impl_type_name(n, source_bytes))
            {
                Some(name) => name,
                None => continue,
            };

            if !exported_type_names.contains(&type_name) {
                continue;
            }

            let body = match impl_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            for i in 0..body.child_count() {
                if let Some(child) = body.child(i as u32) {
                    if child.kind() != "function_item" {
                        continue;
                    }

                    // Check for `pub` visibility modifier
                    let is_pub = (0..child.child_count()).any(|j| {
                        child
                            .child(j as u32)
                            .filter(|c| c.kind() == "visibility_modifier")
                            .and_then(|c| c.utf8_text(source_bytes).ok())
                            .is_some_and(|t| t == "pub")
                    });
                    if !is_pub {
                        continue;
                    }

                    if let Some(method_name) = extract_field_text(&child, source_bytes, "name") {
                        entries.push(ExportEntry::method(
                            method_name,
                            child.start_position().row + 1,
                            child.end_position().row + 1,
                            type_name.clone(),
                        ));
                    }
                }
            }
        }

        entries
    }

    /// Extract exported names from `pub use` declarations in the top-level source.
    fn extract_pub_use_names(&self, source: &str, root_node: Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut results = Vec::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() != "use_declaration" {
                continue;
            }

            let mut is_pub = false;
            let mut content_node: Option<Node> = None;

            let mut child_cursor = child.walk();
            for sub in child.children(&mut child_cursor) {
                match sub.kind() {
                    "visibility_modifier" => {
                        if let Ok(text) = sub.utf8_text(source_bytes)
                            && text == "pub"
                        {
                            is_pub = true;
                        }
                    }
                    "scoped_identifier" | "use_as_clause" | "scoped_use_list" | "identifier" => {
                        content_node = Some(sub);
                    }
                    _ => {}
                }
            }

            if !is_pub {
                continue;
            }

            if let Some(node) = content_node {
                let line = child.start_position().row + 1;
                Self::collect_use_names(source_bytes, node, line, &mut results);
            }
        }

        results
    }

    /// Recursively collect re-exported names from a use clause node.
    fn collect_use_names(
        source_bytes: &[u8],
        node: Node,
        line: usize,
        results: &mut Vec<ExportEntry>,
    ) {
        match node.kind() {
            "scoped_identifier" => {
                // `crate::path::Name` -- the `name` field is the rightmost identifier
                if let Some(name) = extract_field_text(&node, source_bytes, "name") {
                    results.push(ExportEntry::new(name, line, line));
                }
            }
            "use_as_clause" => {
                // `crate::X as Alias` -- use the alias field
                if let Some(name) = extract_field_text(&node, source_bytes, "alias") {
                    results.push(ExportEntry::new(name, line, line));
                }
            }
            "scoped_use_list" => {
                // `crate::path::{A, B}` -- recurse into the list field
                if let Some(list_node) = node.child_by_field_name("list") {
                    let mut cursor = list_node.walk();
                    for item in list_node.children(&mut cursor) {
                        Self::collect_use_names(source_bytes, item, line, results);
                    }
                }
            }
            "use_list" => {
                // Bare `{A, B}` -- recurse into items
                let mut cursor = node.walk();
                for item in node.children(&mut cursor) {
                    Self::collect_use_names(source_bytes, item, line, results);
                }
            }
            "identifier" => {
                // Bare name, e.g. `pub use serde`
                if let Ok(name) = node.utf8_text(source_bytes)
                    && !matches!(name, "self" | "crate" | "super")
                {
                    results.push(ExportEntry::new(name.to_string(), line, line));
                }
            }
            // "use_wildcard", "{", "}", ",", ";", etc. -- skip
            _ => {}
        }
    }
}
