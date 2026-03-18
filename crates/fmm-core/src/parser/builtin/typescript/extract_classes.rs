use std::collections::HashSet;

use streaming_iterator::StreamingIterator;
use tree_sitter::QueryCursor;

use super::TypeScriptParser;
use crate::parser::ExportEntry;

impl TypeScriptParser {
    pub(super) fn extract_class_methods(
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
                if let Some(child) = body.child(i as u32)
                    && child.kind() == "method_definition"
                    && let Some(entry) =
                        Self::extract_method_entry(&class_name, child, source_bytes)
                {
                    entries.push(entry);
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
            if let Some(child) = method_node.child(i as u32)
                && child.kind() == "accessibility_modifier"
            {
                let text = child.utf8_text(source_bytes).unwrap_or("");
                if text == "private" || text == "protected" {
                    return None;
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
}

impl TypeScriptParser {
    pub(super) fn extract_nested_symbols(
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
                        if let Some(c) = child.child(j as u32)
                            && c.kind() == "function_declaration"
                        {
                            found = Some(c);
                            break;
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
            if let Some(child) = decl.child(i as u32)
                && child.kind() == "type_annotation"
            {
                return true;
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
                if let Some(child) = value.child(i as u32)
                    && (child.kind() == "call_expression" || child.kind() == "new_expression")
                {
                    return true;
                }
            }
        }
        false
    }
}
