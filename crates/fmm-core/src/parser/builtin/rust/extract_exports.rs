use super::RustParser;
use super::symbol_metadata::{declaration_kind, rust_entry, rust_method_entry, visibility_for};
use crate::parser::builtin::query_helpers::extract_field_text;
use crate::parser::{DeclarationKind, ExportEntry};
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
                let name_capture = m.captures.iter().find(|c| {
                    let idx = c.index as usize;
                    idx < capture_names.len() && capture_names[idx] == "name"
                });

                if let Some(name) = name_capture
                    && let Ok(text) = name.node.utf8_text(source_bytes)
                {
                    let name_str = text.to_string();
                    let decl = name.node.parent().unwrap_or(name.node);
                    if seen.insert(symbol_key_for_node(None, &name_str, decl))
                        && let Some(kind) = declaration_kind(decl, source_bytes)
                    {
                        exports.push(rust_entry(name_str, decl, source_bytes, kind));
                    }
                }
            }
        }

        self.extract_nested_declarations(root_node, source_bytes, &mut seen, &mut exports);

        // Collect pub use re-exports (always, regardless of binary_crate)
        let pub_use_exports = self.extract_pub_use_names(source, root_node);
        for entry in pub_use_exports {
            if seen.insert(symbol_key(
                entry.parent_class.as_deref(),
                &entry.name,
                entry.start_line,
                entry.end_line,
            )) {
                exports.push(entry);
            }
        }

        // Collect #[macro_export] declarative macros and proc-macro functions
        let macro_exports = self.extract_macro_exports(source, root_node);
        for entry in macro_exports {
            if seen.insert(symbol_key(
                entry.parent_class.as_deref(),
                &entry.name,
                entry.start_line,
                entry.end_line,
            )) {
                exports.push(entry);
            }
        }

        exports.sort_by_key(|e| e.start_line);
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

                    if let Some(method_name) = extract_field_text(&child, source_bytes, "name") {
                        entries.push(rust_method_entry(
                            method_name,
                            child,
                            source_bytes,
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

            let mut visibility = None;
            let mut content_node: Option<Node> = None;

            let mut child_cursor = child.walk();
            for sub in child.children(&mut child_cursor) {
                match sub.kind() {
                    "visibility_modifier" => {
                        visibility = Some(visibility_for(child, source_bytes));
                    }
                    "scoped_identifier" | "use_as_clause" | "scoped_use_list" | "identifier" => {
                        content_node = Some(sub);
                    }
                    _ => {}
                }
            }

            if visibility.is_none() {
                continue;
            }

            if let Some(node) = content_node {
                let line = child.start_position().row + 1;
                Self::collect_use_names(source_bytes, node, line, visibility, &mut results);
            }
        }

        results
    }

    /// Recursively collect re-exported names from a use clause node.
    fn collect_use_names(
        source_bytes: &[u8],
        node: Node,
        line: usize,
        visibility: Option<crate::parser::SymbolVisibility>,
        results: &mut Vec<ExportEntry>,
    ) {
        match node.kind() {
            "scoped_identifier" => {
                // `crate::path::Name` -- the `name` field is the rightmost identifier
                if let Some(name) = extract_field_text(&node, source_bytes, "name") {
                    results.push(Self::use_entry(name, line, visibility));
                }
            }
            "use_as_clause" => {
                // `crate::X as Alias` -- use the alias field
                if let Some(name) = extract_field_text(&node, source_bytes, "alias") {
                    results.push(Self::use_entry(name, line, visibility));
                }
            }
            "scoped_use_list" => {
                // `crate::path::{A, B}` -- recurse into the list field
                if let Some(list_node) = node.child_by_field_name("list") {
                    let mut cursor = list_node.walk();
                    for item in list_node.children(&mut cursor) {
                        Self::collect_use_names(source_bytes, item, line, visibility, results);
                    }
                }
            }
            "use_list" => {
                // Bare `{A, B}` -- recurse into items
                let mut cursor = node.walk();
                for item in node.children(&mut cursor) {
                    Self::collect_use_names(source_bytes, item, line, visibility, results);
                }
            }
            "identifier" => {
                // Bare name, e.g. `pub use serde`
                if let Ok(name) = node.utf8_text(source_bytes)
                    && !matches!(name, "self" | "crate" | "super")
                {
                    results.push(Self::use_entry(name.to_string(), line, visibility));
                }
            }
            // "use_wildcard", "{", "}", ",", ";", etc. -- skip
            _ => {}
        }
    }

    fn extract_nested_declarations(
        &self,
        root_node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            Self::collect_child_declarations(child, source_bytes, seen, exports);
        }
    }

    fn collect_child_declarations(
        node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        Self::collect_named_declaration(node, source_bytes, seen, exports);

        match node.kind() {
            "struct_item" => Self::collect_struct_fields(node, source_bytes, seen, exports),
            "enum_item" => Self::collect_enum_variants(node, source_bytes, seen, exports),
            "trait_item" => Self::collect_trait_methods(node, source_bytes, seen, exports),
            "impl_item" => {
                if let Some(name) = Self::impl_name(node, source_bytes)
                    && seen.insert(symbol_key_for_node(None, &name, node))
                {
                    exports.push(rust_entry(name, node, source_bytes, DeclarationKind::Impl));
                }
            }
            "mod_item" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "declaration_list" {
                        Self::collect_child_declarations(child, source_bytes, seen, exports);
                    }
                }
            }
            "declaration_list" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::collect_child_declarations(child, source_bytes, seen, exports);
                }
            }
            _ => {}
        }
    }

    fn collect_named_declaration(
        node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let Some(kind) = declaration_kind(node, source_bytes) else {
            return;
        };
        let Some(name) = Self::declaration_name(node, source_bytes) else {
            return;
        };
        if seen.insert(symbol_key_for_node(None, &name, node)) {
            exports.push(rust_entry(name, node, source_bytes, kind));
        }
    }

    fn declaration_name(node: Node, source_bytes: &[u8]) -> Option<String> {
        match node.kind() {
            "function_item" | "const_item" | "static_item" | "mod_item" => {
                extract_field_text(&node, source_bytes, "name")
            }
            "struct_item" | "enum_item" | "trait_item" | "type_item" => {
                extract_field_text(&node, source_bytes, "name")
            }
            _ => None,
        }
    }

    fn collect_trait_methods(
        node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let Some(parent_name) = extract_field_text(&node, source_bytes, "name") else {
            return;
        };
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let parent_visibility = visibility_for(node, source_bytes);
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if !matches!(child.kind(), "function_signature_item" | "function_item") {
                continue;
            }
            if let Some(method_name) = extract_field_text(&child, source_bytes, "name") {
                let key = symbol_key_for_node(Some(&parent_name), &method_name, child);
                if seen.insert(key) {
                    let mut entry =
                        rust_method_entry(method_name, child, source_bytes, parent_name.clone());
                    entry.visibility = Some(parent_visibility);
                    exports.push(entry);
                }
            }
        }
    }

    fn collect_struct_fields(
        node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let Some(parent_name) = extract_field_text(&node, source_bytes, "name") else {
            return;
        };
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() != "field_declaration" {
                continue;
            }
            if let Some(field_name) = extract_field_text(&child, source_bytes, "name") {
                let key = symbol_key_for_node(Some(&parent_name), &field_name, child);
                if seen.insert(key) {
                    let mut entry =
                        rust_entry(field_name, child, source_bytes, DeclarationKind::Field);
                    entry.parent_class = Some(parent_name.clone());
                    exports.push(entry);
                }
            }
        }
    }

    fn collect_enum_variants(
        node: Node,
        source_bytes: &[u8],
        seen: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
    ) {
        let Some(parent_name) = extract_field_text(&node, source_bytes, "name") else {
            return;
        };
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() != "enum_variant" {
                continue;
            }
            if let Some(variant_name) = extract_field_text(&child, source_bytes, "name") {
                let key = symbol_key_for_node(Some(&parent_name), &variant_name, child);
                if seen.insert(key) {
                    let mut entry =
                        rust_entry(variant_name, child, source_bytes, DeclarationKind::Variant);
                    entry.parent_class = Some(parent_name.clone());
                    exports.push(entry);
                }
            }
        }
    }

    fn impl_name(node: Node, source_bytes: &[u8]) -> Option<String> {
        let type_name = node
            .child_by_field_name("type")
            .and_then(|type_node| Self::impl_type_name(type_node, source_bytes))?;
        let trait_name = node
            .child_by_field_name("trait")
            .and_then(|trait_node| trait_node.utf8_text(source_bytes).ok());
        Some(match trait_name {
            Some(trait_name) => format!("impl {} for {}", trait_name, type_name),
            None => format!("impl {}", type_name),
        })
    }

    fn use_entry(
        name: String,
        line: usize,
        visibility: Option<crate::parser::SymbolVisibility>,
    ) -> ExportEntry {
        let mut entry = ExportEntry::new(name, line, line);
        entry.visibility = visibility;
        entry.signature = Some(entry.name.clone());
        entry.declaration_kind = Some(DeclarationKind::Module);
        entry
    }
}

fn symbol_key_for_node(parent: Option<&str>, name: &str, node: Node) -> String {
    symbol_key(parent, name, node.start_byte(), node.end_byte())
}

fn symbol_key(parent: Option<&str>, name: &str, start: usize, end: usize) -> String {
    match parent {
        Some(parent) => format!("{parent}::{name}@{start}:{end}"),
        None => format!("{name}@{start}:{end}"),
    }
}
