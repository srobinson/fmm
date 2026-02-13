use super::query_helpers::top_level_ancestor;
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct RustParser {
    parser: TSParser,
    export_queries: Vec<Query>,
    unsafe_query: Query,
    trait_impl_queries: Vec<Query>,
    lifetime_query: Query,
    async_query: Query,
    derive_query: Query,
}

impl RustParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Rust language: {}", e))?;

        let export_query_strs = [
            "(function_item (visibility_modifier) @vis name: (identifier) @name)",
            "(struct_item (visibility_modifier) @vis name: (type_identifier) @name)",
            "(enum_item (visibility_modifier) @vis name: (type_identifier) @name)",
            "(trait_item (visibility_modifier) @vis name: (type_identifier) @name)",
            "(type_item (visibility_modifier) @vis name: (type_identifier) @name)",
            "(const_item (visibility_modifier) @vis name: (identifier) @name)",
            "(static_item (visibility_modifier) @vis name: (identifier) @name)",
            "(mod_item (visibility_modifier) @vis name: (identifier) @name)",
        ];

        let export_queries: Vec<Query> = export_query_strs
            .iter()
            .map(|q| Query::new(&language, q))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to compile export query: {}", e))?;

        let unsafe_query = Query::new(&language, "(unsafe_block) @block")
            .map_err(|e| anyhow::anyhow!("Failed to compile unsafe query: {}", e))?;

        let trait_impl_queries = vec![
            Query::new(
                &language,
                "(impl_item trait: (type_identifier) @trait type: (type_identifier) @type)",
            )
            .map_err(|e| anyhow::anyhow!("Failed to compile trait_impl query: {}", e))?,
            Query::new(
                &language,
                "(impl_item trait: (scoped_type_identifier) @trait type: (type_identifier) @type)",
            )
            .map_err(|e| anyhow::anyhow!("Failed to compile scoped trait_impl query: {}", e))?,
        ];

        let lifetime_query = Query::new(&language, "(lifetime (identifier) @name)")
            .map_err(|e| anyhow::anyhow!("Failed to compile lifetime query: {}", e))?;

        let async_query = Query::new(&language, "(function_item (function_modifiers) @mods)")
            .map_err(|e| anyhow::anyhow!("Failed to compile async query: {}", e))?;

        let derive_query = Query::new(
            &language,
            "(attribute_item (attribute (identifier) @attr_name arguments: (token_tree) @args))",
        )
        .map_err(|e| anyhow::anyhow!("Failed to compile derive query: {}", e))?;

        Ok(Self {
            parser,
            export_queries,
            unsafe_query,
            trait_impl_queries,
            lifetime_query,
            async_query,
            derive_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        for query in &self.export_queries {
            let capture_names = query.capture_names();
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                let vis_capture = m.captures.iter().find(|c| {
                    let idx = c.index as usize;
                    idx < capture_names.len() && capture_names[idx] == "vis"
                });
                let name_capture = m.captures.iter().find(|c| {
                    let idx = c.index as usize;
                    idx < capture_names.len() && capture_names[idx] == "name"
                });

                if let (Some(vis), Some(name)) = (vis_capture, name_capture) {
                    if let Ok(vis_text) = vis.node.utf8_text(source_bytes) {
                        if vis_text != "pub" {
                            continue;
                        }
                    }
                    if let Ok(text) = name.node.utf8_text(source_bytes) {
                        let name_str = text.to_string();
                        if seen.insert(name_str.clone()) {
                            let decl = top_level_ancestor(name.node);
                            exports.push(ExportEntry::new(
                                name_str,
                                decl.start_position().row + 1,
                                decl.end_position().row + 1,
                            ));
                        }
                    }
                }
            }
        }

        exports.sort_by(|a, b| a.name.cmp(&b.name));
        exports.dedup_by(|a, b| a.name == b.name);
        exports
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        let roots = self.extract_use_roots(source_bytes, root_node);
        for root in roots {
            if !Self::is_local_or_std(&root) && seen.insert(root.clone()) {
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
                            if !Self::is_local_or_std(&name) && seen.insert(name.clone()) {
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

    fn is_local_or_std(name: &str) -> bool {
        matches!(name, "self" | "crate" | "super" | "std" | "core" | "alloc")
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut deps = Vec::new();
        let source_bytes = source.as_bytes();
        let roots = self.extract_use_roots(source_bytes, root_node);
        for root in roots {
            if ["crate", "super"].contains(&root.as_str()) && seen.insert(root.clone()) {
                deps.push(root);
            }
        }
        deps.sort();
        deps.dedup();
        deps
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
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.unsafe_query, root_node, source_bytes);
        let mut count = 0;
        while iter.next().is_some() {
            count += 1;
        }
        count
    }

    fn extract_trait_impls(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
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

    fn extract_lifetimes(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
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

    fn count_async_functions(&self, source: &str, root_node: tree_sitter::Node) -> usize {
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

    fn extract_derives(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
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
}

impl Parser for RustParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust source"))?;

        let root_node = tree.root_node();

        let exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let loc = source.lines().count();

        let unsafe_count = self.count_unsafe_blocks(source, root_node);
        let derives = self.extract_derives(source, root_node);
        let trait_impls = self.extract_trait_impls(source, root_node);
        let lifetimes = self.extract_lifetimes(source, root_node);
        let async_count = self.count_async_functions(source, root_node);

        let has_custom = unsafe_count > 0
            || !derives.is_empty()
            || !trait_impls.is_empty()
            || !lifetimes.is_empty()
            || async_count > 0;

        let custom_fields = if !has_custom {
            None
        } else {
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
            if !trait_impls.is_empty() {
                fields.insert(
                    "trait_impls".to_string(),
                    serde_json::Value::Array(
                        trait_impls
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            if !lifetimes.is_empty() {
                fields.insert(
                    "lifetimes".to_string(),
                    serde_json::Value::Array(
                        lifetimes
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            if async_count > 0 {
                fields.insert(
                    "async_functions".to_string(),
                    serde_json::Value::Number(async_count.into()),
                );
            }
            Some(fields)
        };

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
            },
            custom_fields,
        })
    }

    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_pub_functions() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub fn hello() {}\nfn private() {}\npub fn world() {}";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"hello".to_string()));
        assert!(result
            .metadata
            .export_names()
            .contains(&"world".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"private".to_string()));
    }

    #[test]
    fn parse_rust_pub_structs_and_enums() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub struct Foo {}\npub enum Bar { A, B }\nstruct Private {}";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.export_names().contains(&"Foo".to_string()));
        assert!(result.metadata.export_names().contains(&"Bar".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"Private".to_string()));
    }

    #[test]
    fn parse_rust_use_imports() {
        let mut parser = RustParser::new().unwrap();
        let source =
            "use std::collections::HashMap;\nuse anyhow::Result;\nuse crate::config::Config;";
        let result = parser.parse(source).unwrap();
        assert!(!result.metadata.imports.contains(&"std".to_string()));
        assert!(result.metadata.imports.contains(&"anyhow".to_string()));
        assert!(!result.metadata.imports.contains(&"crate".to_string()));
    }

    #[test]
    fn parse_rust_extern_crate() {
        let mut parser = RustParser::new().unwrap();
        let source = "extern crate serde;\nextern crate log;\nuse serde::Deserialize;";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"serde".to_string()));
        assert!(result.metadata.imports.contains(&"log".to_string()));
    }

    #[test]
    fn parse_rust_filters_std_core_alloc() {
        let mut parser = RustParser::new().unwrap();
        let source = "use std::io;\nuse core::fmt;\nuse alloc::vec::Vec;\nuse tokio::runtime;";
        let result = parser.parse(source).unwrap();
        assert!(!result.metadata.imports.contains(&"std".to_string()));
        assert!(!result.metadata.imports.contains(&"core".to_string()));
        assert!(!result.metadata.imports.contains(&"alloc".to_string()));
        assert!(result.metadata.imports.contains(&"tokio".to_string()));
    }

    #[test]
    fn parse_rust_pub_crate_excluded() {
        let mut parser = RustParser::new().unwrap();
        let source =
            "pub fn visible() {}\npub(crate) fn internal() {}\npub(super) fn parent_only() {}";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"visible".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"internal".to_string()));
        assert!(!result
            .metadata
            .export_names()
            .contains(&"parent_only".to_string()));
    }

    #[test]
    fn parse_rust_crate_deps() {
        let mut parser = RustParser::new().unwrap();
        let source = "use crate::config::Config;\nuse super::utils;";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.dependencies.contains(&"crate".to_string()));
        assert!(result.metadata.dependencies.contains(&"super".to_string()));
    }

    #[test]
    fn rust_custom_fields_unsafe() {
        let mut parser = RustParser::new().unwrap();
        let source = "fn foo() { unsafe { std::ptr::null() }; }\nfn bar() { unsafe { 1 }; }";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("unsafe_blocks").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn rust_custom_fields_derives() {
        let mut parser = RustParser::new().unwrap();
        let source = "#[derive(Debug, Clone, Serialize)]\npub struct Foo {}";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let derives = fields.get("derives").unwrap().as_array().unwrap();
        let names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Debug"));
        assert!(names.contains(&"Clone"));
        assert!(names.contains(&"Serialize"));
    }

    #[test]
    fn rust_no_custom_fields_when_clean() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub fn hello() {}";
        let result = parser.parse(source).unwrap();
        assert!(result.custom_fields.is_none());
    }

    #[test]
    fn parse_rust_loc() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub fn hello() {\n    42\n}\n";
        let result = parser.parse(source).unwrap();
        assert_eq!(result.metadata.loc, 3);
    }

    #[test]
    fn rust_custom_fields_trait_impls() {
        let mut parser = RustParser::new().unwrap();
        let source = "struct Foo {}\nimpl Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n}\nimpl Clone for Foo {\n    fn clone(&self) -> Self { Foo {} }\n}";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
        let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Clone for Foo"));
        assert!(names.contains(&"Display for Foo"));
    }

    #[test]
    fn rust_custom_fields_lifetimes() {
        let mut parser = RustParser::new().unwrap();
        let source = "pub struct Ref<'a> {\n    data: &'a str,\n}";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
        let names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"'a"));
    }

    #[test]
    fn rust_custom_fields_async_functions() {
        let mut parser = RustParser::new().unwrap();
        let source = "async fn fetch() {}\nasync fn process() {}\nfn sync_fn() {}";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("async_functions").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn rust_scoped_trait_impl() {
        let mut parser = RustParser::new().unwrap();
        let source = "struct Foo {}\nimpl std::fmt::Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }\n}";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.unwrap();
        let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
        let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"Display for Foo"));
    }

    #[test]
    fn rust_anonymous_lifetime_filtered() {
        let mut parser = RustParser::new().unwrap();
        let source = "fn foo(x: &'_ str) {}";
        let result = parser.parse(source).unwrap();
        if let Some(fields) = result.custom_fields {
            if let Some(lts) = fields.get("lifetimes") {
                let names: Vec<&str> = lts
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                assert!(!names.contains(&"'_"));
            }
        }
    }
}
