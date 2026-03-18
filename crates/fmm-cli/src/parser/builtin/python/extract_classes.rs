use super::PythonParser;
use crate::parser::ExportEntry;
use std::collections::HashSet;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

impl PythonParser {
    /// ALP-769: extract public methods from exported classes.
    /// Returns `ExportEntry` items with `parent_class` set to the class name.
    ///
    /// Public heuristic: include if name does not start with `_`, OR name is `__init__`.
    /// All other dunder methods (`__str__`, `__repr__`, etc.) are skipped.
    /// Decorated methods (`@property`, `@staticmethod`, etc.) are included.
    pub(super) fn extract_class_methods(
        &self,
        source: &str,
        root_node: Node,
        exported_class_names: &HashSet<String>,
    ) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut entries = Vec::new();

        let class_name_idx = self
            .class_method_query
            .capture_index_for_name("class_name")
            .unwrap_or(0);
        let class_idx = self
            .class_method_query
            .capture_index_for_name("class")
            .unwrap_or(1);

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.class_method_query, root_node, source_bytes);

        while let Some(m) = iter.next() {
            let mut class_node: Option<Node> = None;
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
                    match child.kind() {
                        "function_definition" => {
                            if let Some(entry) =
                                Self::extract_python_method_entry(&class_name, child, source_bytes)
                            {
                                entries.push(entry);
                            }
                        }
                        "decorated_definition" => {
                            // Find the function_definition inside the decorated_definition
                            for j in 0..child.child_count() {
                                if let Some(inner) = child.child(j as u32) {
                                    if inner.kind() == "function_definition" {
                                        if let Some(mut entry) = Self::extract_python_method_entry(
                                            &class_name,
                                            inner,
                                            source_bytes,
                                        ) {
                                            // Use the decorated_definition range to include decorator lines
                                            entry.start_line = child.start_position().row + 1;
                                            entry.end_line = child.end_position().row + 1;
                                            entries.push(entry);
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        entries
    }

    /// Extract a single function_definition node as an ExportEntry.
    /// Returns None for private methods (leading underscore), except `__init__`.
    fn extract_python_method_entry(
        class_name: &str,
        method_node: Node,
        source_bytes: &[u8],
    ) -> Option<ExportEntry> {
        let name_node = method_node.child_by_field_name("name")?;
        let method_name = name_node.utf8_text(source_bytes).ok()?.to_string();

        // Include public methods and __init__; skip all other underscore-prefixed names
        if method_name.starts_with('_') && method_name != "__init__" {
            return None;
        }

        Some(ExportEntry::method(
            method_name,
            method_node.start_position().row + 1,
            method_node.end_position().row + 1,
            class_name.to_string(),
        ))
    }
}
