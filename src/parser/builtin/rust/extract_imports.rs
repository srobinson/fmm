use super::{rust_use_path_to_dep, RustParser};
use std::collections::HashSet;

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
}
