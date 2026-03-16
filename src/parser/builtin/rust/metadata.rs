use super::RustParser;
use std::collections::HashSet;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

impl RustParser {
    pub(super) fn count_unsafe_blocks(&self, source: &str, root_node: Node) -> usize {
        let source_bytes = source.as_bytes();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.unsafe_query, root_node, source_bytes);
        let mut count = 0;
        while iter.next().is_some() {
            count += 1;
        }
        count
    }

    pub(super) fn extract_trait_impls(&self, source: &str, root_node: Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut impls = Vec::new();
        let source_bytes = source.as_bytes();

        for query in &self.trait_impl_queries {
            let capture_names = query.capture_names();
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                let trait_name = m
                    .captures
                    .iter()
                    .find(|c| {
                        let idx = c.index as usize;
                        idx < capture_names.len() && capture_names[idx] == "trait"
                    })
                    .and_then(|c| c.node.utf8_text(source_bytes).ok());
                let type_name = m
                    .captures
                    .iter()
                    .find(|c| {
                        let idx = c.index as usize;
                        idx < capture_names.len() && capture_names[idx] == "type"
                    })
                    .and_then(|c| c.node.utf8_text(source_bytes).ok());

                if let (Some(t), Some(ty)) = (trait_name, type_name) {
                    let trait_short = t.rsplit("::").next().unwrap_or(t);
                    let entry = format!("{} for {}", trait_short, ty);
                    if seen.insert(entry.clone()) {
                        impls.push(entry);
                    }
                }
            }
        }

        impls.sort();
        impls
    }

    pub(super) fn extract_lifetimes(&self, source: &str, root_node: Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut lifetimes = Vec::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.lifetime_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if text == "_" {
                        continue;
                    }
                    let lt = format!("'{}", text);
                    if seen.insert(lt.clone()) {
                        lifetimes.push(lt);
                    }
                }
            }
        }

        lifetimes.sort();
        lifetimes
    }

    pub(super) fn count_async_functions(&self, source: &str, root_node: Node) -> usize {
        let source_bytes = source.as_bytes();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.async_query, root_node, source_bytes);
        let mut count = 0;
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    if text.contains("async") {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    pub(super) fn extract_derives(&self, source: &str, root_node: Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut derives = Vec::new();
        let source_bytes = source.as_bytes();
        let capture_names = self.derive_query.capture_names();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.derive_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            let attr_name = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "attr_name"
            });
            let args = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "args"
            });

            if let (Some(name_capture), Some(args_capture)) = (attr_name, args) {
                if let Ok(name) = name_capture.node.utf8_text(source_bytes) {
                    if name == "derive" {
                        if let Ok(args_text) = args_capture.node.utf8_text(source_bytes) {
                            let inner = args_text.trim_start_matches('(').trim_end_matches(')');
                            for d in inner.split(',') {
                                let d = d.trim().to_string();
                                if !d.is_empty() && seen.insert(d.clone()) {
                                    derives.push(d);
                                }
                            }
                        }
                    }
                }
            }
        }

        derives.sort();
        derives.dedup();
        derives
    }

    /// Extract pub function names at module scope for `function_index`.
    ///
    /// Uses the first export query which matches `source_file > function_item`
    /// with a visibility_modifier. Returns function names only (not structs,
    /// enums, traits, or impl methods).
    pub(super) fn extract_function_names(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Vec<String> {
        use streaming_iterator::StreamingIterator;

        let source_bytes = source.as_bytes();
        let mut names = Vec::new();
        let query = &self.export_queries[0]; // function_item query

        let mut cursor = tree_sitter::QueryCursor::new();
        let name_idx = query.capture_index_for_name("name").unwrap_or(u32::MAX);
        let mut iter = cursor.matches(query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == name_idx {
                    if let Ok(text) = cap.node.utf8_text(source_bytes) {
                        names.push(text.to_string());
                    }
                }
            }
        }

        names
    }
}
