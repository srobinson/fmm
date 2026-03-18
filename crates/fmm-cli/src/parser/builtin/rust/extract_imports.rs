use super::{rust_use_path_to_dep, RustParser};
use std::collections::{HashMap, HashSet};

impl RustParser {
    pub(super) fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        let roots = self.extract_use_roots(source_bytes, root_node);
        for root in roots {
            if !Self::is_local_path(&root) && seen.insert(root.clone()) {
                imports.push(root);
            }
        }

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() == "extern_crate_declaration" {
                let mut inner = child.walk();
                for c in child.children(&mut inner) {
                    if c.kind() == "identifier" {
                        if let Ok(name) = c.utf8_text(source_bytes) {
                            let name = name.to_string();
                            if !Self::is_local_path(&name) && seen.insert(name.clone()) {
                                imports.push(name);
                            }
                        }
                    }
                }
            }
        }

        imports.sort();
        imports.dedup();
        imports
    }

    fn is_local_path(name: &str) -> bool {
        matches!(name, "self" | "crate" | "super")
    }

    pub(super) fn extract_dependencies(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut deps = Vec::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() != "use_declaration" {
                continue;
            }
            for dep in self.use_declaration_deps(source_bytes, child) {
                if seen.insert(dep.clone()) {
                    deps.push(dep);
                }
            }
        }

        deps.sort();
        deps.dedup();
        deps
    }

    /// Extract normalized dep strings from a single `use_declaration` node.
    /// Returns `crate::X` or `../X` strings for internal dependencies;
    /// returns nothing for external crate imports.
    fn use_declaration_deps(&self, source_bytes: &[u8], node: tree_sitter::Node) -> Vec<String> {
        let mut cursor = node.walk();
        let mut results = Vec::new();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" => {
                    if let Ok(raw) = child.utf8_text(source_bytes) {
                        if let Some(dep) = rust_use_path_to_dep(raw) {
                            results.push(dep);
                        }
                    }
                }
                "scoped_use_list" => {
                    // e.g. `crate::parser::{builtin, search}` -- emit the path prefix
                    let mut sub = child.walk();
                    for sub_child in child.children(&mut sub) {
                        match sub_child.kind() {
                            "scoped_identifier" | "crate" | "super" => {
                                if let Ok(raw) = sub_child.utf8_text(source_bytes) {
                                    if let Some(dep) = rust_use_path_to_dep(raw) {
                                        results.push(dep);
                                    }
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                "use_wildcard" => {
                    // e.g. `use crate::parser::*` -- strip trailing ::* to get dep path
                    if let Ok(raw) = child.utf8_text(source_bytes) {
                        let prefix = raw.strip_suffix("::*").unwrap_or(raw);
                        if let Some(dep) = rust_use_path_to_dep(prefix) {
                            results.push(dep);
                        }
                    }
                }
                _ => {}
            }
        }

        results
    }

    fn extract_use_roots(&self, source_bytes: &[u8], root_node: tree_sitter::Node) -> Vec<String> {
        let mut roots = Vec::new();
        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() == "use_declaration" {
                if let Some(root_name) = self.use_declaration_root(source_bytes, child) {
                    roots.push(root_name);
                }
            }
        }
        roots
    }

    fn use_declaration_root(&self, source_bytes: &[u8], node: tree_sitter::Node) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "scoped_use_list" | "use_wildcard" => {
                    return self.leftmost_path_leaf(source_bytes, child);
                }
                "identifier" => {
                    return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
                }
                _ => {}
            }
        }
        None
    }

    fn leftmost_path_leaf(&self, source_bytes: &[u8], node: tree_sitter::Node) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" => {
                    return self.leftmost_path_leaf(source_bytes, child);
                }
                "identifier" | "crate" | "super" | "self" => {
                    return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
                }
                _ => {}
            }
        }
        None
    }

    /// Extract named and namespace imports from all `use` declarations.
    ///
    /// Returns `(named_imports, namespace_imports)`:
    /// - `use path::Symbol` -> `named_imports["path"] = ["Symbol"]`
    /// - `use path::{A, B}` -> `named_imports["path"] = ["A", "B"]`
    /// - `use path::*` -> `namespace_imports.push("path")`
    /// - `use path::Symbol as Alias` -> `named_imports["path"] = ["Symbol"]` (original name)
    /// - Nested groups handled recursively
    pub(super) fn extract_named_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (HashMap<String, Vec<String>>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut named: HashMap<String, Vec<String>> = HashMap::new();
        let mut namespace: Vec<String> = Vec::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() != "use_declaration" {
                continue;
            }
            // Find the import node (skip `use` keyword and `;`)
            let mut inner = child.walk();
            for use_child in child.children(&mut inner) {
                match use_child.kind() {
                    "scoped_identifier" | "scoped_use_list" | "use_wildcard" | "use_as_clause"
                    | "identifier" => {
                        Self::collect_use_item(
                            source_bytes,
                            use_child,
                            "",
                            &mut named,
                            &mut namespace,
                        );
                    }
                    _ => {}
                }
            }
        }

        (named, namespace)
    }

    /// Recursively collect named/namespace imports from a use tree node.
    ///
    /// `prefix` accumulates the module path as we descend through nested
    /// `scoped_use_list` nodes. Leaf nodes (identifiers, wildcards) combine
    /// the prefix with their local path to produce the final import entry.
    fn collect_use_item(
        source_bytes: &[u8],
        node: tree_sitter::Node,
        prefix: &str,
        named: &mut HashMap<String, Vec<String>>,
        namespace: &mut Vec<String>,
    ) {
        match node.kind() {
            "scoped_identifier" => {
                // e.g. `std::collections::HashMap`
                // path field = module path, name field = imported symbol
                let path_node = node.child_by_field_name("path");
                let name_node = node.child_by_field_name("name");
                if let (Some(path), Some(name)) = (path_node, name_node) {
                    let path_text = path.utf8_text(source_bytes).unwrap_or("");
                    let full_path = Self::join_prefix(prefix, path_text);
                    if let Ok(name_text) = name.utf8_text(source_bytes) {
                        named
                            .entry(full_path)
                            .or_default()
                            .push(name_text.to_string());
                    }
                }
            }
            "scoped_use_list" => {
                // e.g. `crate::module::{A, B}` or nested `{sub::X, other::Y}`
                let path_node = node.child_by_field_name("path");
                let list_node = node.child_by_field_name("list");

                let path_text = path_node
                    .and_then(|p| p.utf8_text(source_bytes).ok())
                    .unwrap_or("");
                let full_prefix = Self::join_prefix(prefix, path_text);

                if let Some(list) = list_node {
                    let mut cursor = list.walk();
                    for child in list.children(&mut cursor) {
                        Self::collect_use_item(source_bytes, child, &full_prefix, named, namespace);
                    }
                }
            }
            "use_wildcard" => {
                // e.g. `crate::module::*` or bare `*` inside a use list
                let text = node.utf8_text(source_bytes).unwrap_or("");
                if text == "*" {
                    // Bare wildcard inside a use list: namespace is the prefix
                    if !prefix.is_empty() {
                        namespace.push(prefix.to_string());
                    }
                } else {
                    let module_path = text.strip_suffix("::*").unwrap_or(text);
                    namespace.push(Self::join_prefix(prefix, module_path));
                }
            }
            "use_as_clause" => {
                // e.g. `Symbol as Alias` -- store original name, not alias
                if let Some(path) = node.child_by_field_name("path") {
                    Self::collect_use_item(source_bytes, path, prefix, named, namespace);
                }
            }
            "identifier" => {
                if let Ok(name) = node.utf8_text(source_bytes) {
                    if prefix.is_empty() {
                        // Bare `use serde;` -> namespace import
                        namespace.push(name.to_string());
                    } else {
                        // Inside a use list: `{A, B}` with prefix
                        named
                            .entry(prefix.to_string())
                            .or_default()
                            .push(name.to_string());
                    }
                }
            }
            "self" => {
                // `use module::{self}` -> namespace import of the module
                if !prefix.is_empty() {
                    namespace.push(prefix.to_string());
                }
            }
            _ => {}
        }
    }

    /// Join a prefix and a segment with `::`, handling empty prefix.
    fn join_prefix(prefix: &str, segment: &str) -> String {
        if prefix.is_empty() {
            segment.to_string()
        } else if segment.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}::{segment}")
        }
    }
}
