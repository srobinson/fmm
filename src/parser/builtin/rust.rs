use crate::parser::{Metadata, Parser};
use anyhow::Result;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct RustParser {
    parser: TSParser,
    language: Language,
}

impl RustParser {
    pub fn new() -> Result<Self> {
        let language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Rust language: {}", e))?;

        Ok(Self { parser, language })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        let queries = [
            // pub fn foo()
            "(function_item (visibility_modifier) name: (identifier) @name)",
            // pub struct Foo
            "(struct_item (visibility_modifier) name: (type_identifier) @name)",
            // pub enum Foo
            "(enum_item (visibility_modifier) name: (type_identifier) @name)",
            // pub trait Foo
            "(trait_item (visibility_modifier) name: (type_identifier) @name)",
            // pub type Foo = ...
            "(type_item (visibility_modifier) name: (type_identifier) @name)",
            // pub const FOO: ...
            "(const_item (visibility_modifier) name: (identifier) @name)",
            // pub static FOO: ...
            "(static_item (visibility_modifier) name: (identifier) @name)",
            // pub mod foo
            "(mod_item (visibility_modifier) name: (identifier) @name)",
        ];

        for query_str in queries {
            if let Ok(query) = Query::new(&self.language, query_str) {
                let mut cursor = QueryCursor::new();
                let mut iter = cursor.matches(&query, root_node, source_bytes);
                while let Some(m) = iter.next() {
                    for capture in m.captures {
                        if let Ok(text) = capture.node.utf8_text(source_bytes) {
                            let name = text.to_string();
                            if !exports.contains(&name) {
                                exports.push(name);
                            }
                        }
                    }
                }
            }
        }

        exports.sort();
        exports.dedup();
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();
        let roots = self.extract_use_roots(source_bytes, root_node);
        for root in roots {
            if !["self", "crate", "super"].contains(&root.as_str()) && !imports.contains(&root) {
                imports.push(root);
            }
        }
        imports.sort();
        imports.dedup();
        imports
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut deps = Vec::new();
        let source_bytes = source.as_bytes();
        let roots = self.extract_use_roots(source_bytes, root_node);
        for root in roots {
            if ["crate", "super"].contains(&root.as_str()) && !deps.contains(&root) {
                deps.push(root);
            }
        }
        deps.sort();
        deps.dedup();
        deps
    }

    /// Walk all use_declaration nodes and extract the root crate/module name
    /// by recursively descending into scoped_identifier until we hit a leaf.
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

    /// Find the leftmost leaf of a use_declaration's path (the crate name).
    fn use_declaration_root(&self, source_bytes: &[u8], node: tree_sitter::Node) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "scoped_use_list" => {
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

    /// Recursively descend the leftmost path: child to find the root identifier/crate node.
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

    fn count_unsafe_blocks(&self, source: &str, root_node: tree_sitter::Node) -> usize {
        let source_bytes = source.as_bytes();
        let query_str = "(unsafe_block) @block";
        if let Ok(query) = Query::new(&self.language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            let mut count = 0;
            while iter.next().is_some() {
                count += 1;
            }
            count
        } else {
            0
        }
    }

    fn extract_derives(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut derives = Vec::new();
        let source_bytes = source.as_bytes();

        // Match attribute_item containing derive
        let query_str =
            "(attribute_item (attribute (identifier) @attr_name arguments: (token_tree) @args))";
        if let Ok(query) = Query::new(&self.language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                // Check if attr_name is "derive"
                let attr_name = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "attr_name");
                let args = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "args");

                if let (Some(name_capture), Some(args_capture)) = (attr_name, args) {
                    if let Ok(name) = name_capture.node.utf8_text(source_bytes) {
                        if name == "derive" {
                            if let Ok(args_text) = args_capture.node.utf8_text(source_bytes) {
                                // Parse "(Debug, Clone, Serialize)" -> ["Debug", "Clone", "Serialize"]
                                let inner = args_text.trim_start_matches('(').trim_end_matches(')');
                                for d in inner.split(',') {
                                    let d = d.trim().to_string();
                                    if !d.is_empty() && !derives.contains(&d) {
                                        derives.push(d);
                                    }
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
}

impl Parser for RustParser {
    fn parse(&mut self, source: &str) -> Result<Metadata> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust source"))?;

        let root_node = tree.root_node();

        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        Ok(Metadata {
            exports,
            imports,
            dependencies,
            loc,
        })
    }

    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn custom_fields(&self, source: &str) -> Option<HashMap<String, serde_json::Value>> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = TSParser::new();
        if parser.set_language(&language).is_err() {
            return None;
        }
        let tree = parser.parse(source, None)?;
        let root_node = tree.root_node();

        let unsafe_count = self.count_unsafe_blocks(source, root_node);
        let derives = self.extract_derives(source, root_node);

        if unsafe_count == 0 && derives.is_empty() {
            return None;
        }

        let mut fields = HashMap::new();
        if unsafe_count > 0 {
            fields.insert(
                "unsafe_blocks".to_string(),
                serde_json::Value::Number(unsafe_count.into()),
            );
        }
        if !derives.is_empty() {
            fields.insert(
                "derives".to_string(),
                serde_json::Value::Array(
                    derives.into_iter().map(serde_json::Value::String).collect(),
                ),
            );
        }
        Some(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_pub_functions() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub fn hello() {}\nfn private() {}\npub fn world() {}";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.exports.contains(&"hello".to_string()));
        assert!(metadata.exports.contains(&"world".to_string()));
        assert!(!metadata.exports.contains(&"private".to_string()));
    }

    #[test]
    fn parse_rust_pub_structs_and_enums() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub struct Foo {}\npub enum Bar { A, B }\nstruct Private {}";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.exports.contains(&"Foo".to_string()));
        assert!(metadata.exports.contains(&"Bar".to_string()));
        assert!(!metadata.exports.contains(&"Private".to_string()));
    }

    #[test]
    fn parse_rust_use_imports() {
        let mut parser = RustParser::new().unwrap();
        let source =
            "use std::collections::HashMap;\nuse anyhow::Result;\nuse crate::config::Config;";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.imports.contains(&"std".to_string()));
        assert!(metadata.imports.contains(&"anyhow".to_string()));
        assert!(!metadata.imports.contains(&"crate".to_string()));
    }

    #[test]
    fn parse_rust_crate_deps() {
        let mut parser = RustParser::new().unwrap();
        let source = "use crate::config::Config;\nuse super::utils;";
        let metadata = parser.parse(source).unwrap();
        assert!(metadata.dependencies.contains(&"crate".to_string()));
        assert!(metadata.dependencies.contains(&"super".to_string()));
    }

    #[test]
    fn rust_custom_fields_unsafe() {
        let parser = RustParser::new().unwrap();
        let source = "fn foo() { unsafe { std::ptr::null() }; }\nfn bar() { unsafe { 1 }; }";
        let fields = parser.custom_fields(source);
        assert!(fields.is_some());
        let fields = fields.unwrap();
        assert_eq!(fields.get("unsafe_blocks").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn rust_custom_fields_derives() {
        let parser = RustParser::new().unwrap();
        let source = "#[derive(Debug, Clone, Serialize)]\npub struct Foo {}";
        let fields = parser.custom_fields(source);
        assert!(fields.is_some());
        let fields = fields.unwrap();
        let derives = fields.get("derives").unwrap().as_array().unwrap();
        let names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Debug"));
        assert!(names.contains(&"Clone"));
        assert!(names.contains(&"Serialize"));
    }

    #[test]
    fn rust_no_custom_fields_when_clean() {
        let parser = RustParser::new().unwrap();
        let source = "pub fn hello() {}";
        assert!(parser.custom_fields(source).is_none());
    }

    #[test]
    fn parse_rust_loc() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub fn hello() {\n    42\n}\n";
        let metadata = parser.parse(source).unwrap();
        assert_eq!(metadata.loc, 3);
    }
}
