mod extract_exports;
mod extract_imports;
mod extract_macros;
mod metadata;

#[cfg(test)]
mod tests;

use super::query_helpers::{compile_query, make_parser};
use crate::parser::{Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tree_sitter::{Language, Parser as TSParser, Query};

/// Convert a raw Rust use-path that starts with `crate::` or `super::` into
/// the normalized dep string that `dep_matches()` understands.
///
/// * `crate::config::Config` -> `Some("crate::config")`  (PascalCase tail dropped)
/// * `super::utils` -> `Some("../utils")`
/// * `std::collections::HashMap` -> `None` (external crate, not a dep)
fn rust_use_path_to_dep(raw: &str) -> Option<String> {
    if !raw.starts_with("crate::") && !raw.starts_with("super::") {
        return None;
    }

    // Strip trailing PascalCase segment: Rust convention is types/traits are PascalCase,
    // modules are snake_case. If the last :: segment starts with uppercase, it's a type.
    let path = if let Some(sep_pos) = raw.rfind("::") {
        let last = &raw[sep_pos + 2..];
        if last
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            &raw[..sep_pos]
        } else {
            raw
        }
    } else {
        raw
    };

    if let Some(rest) = path.strip_prefix("super::") {
        // super::X::Y -> ../X/Y
        Some(format!("../{}", rest.replace("::", "/")))
    } else if path.starts_with("crate::") {
        // crate::X -> keep as-is for dep_matches() crate:: fallback
        Some(path.to_string())
    } else {
        // bare "crate" or "super" with no sub-path -- skip
        None
    }
}

pub struct RustParser {
    parser: TSParser,
    export_queries: Vec<Query>,
    all_item_queries: Vec<Query>,
    unsafe_query: Query,
    trait_impl_queries: Vec<Query>,
    lifetime_query: Query,
    async_query: Query,
    derive_query: Query,
    /// ALP-770: finds impl blocks for public method extraction
    impl_query: Query,
}

impl RustParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let parser = make_parser(&language, "Rust")?;

        let export_query_strs = [
            // Anchored to source_file so that pub fn inside impl blocks are NOT captured here.
            // impl block methods are extracted separately with parent_class set (ALP-770).
            "(source_file (function_item (visibility_modifier) @vis name: (identifier) @name))",
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
            .map(|q| compile_query(&language, q, "export"))
            .collect::<Result<Vec<_>>>()?;

        // Queries that match all items regardless of visibility (for binary crates)
        let all_item_query_strs = [
            "(function_item name: (identifier) @name)",
            "(struct_item name: (type_identifier) @name)",
            "(enum_item name: (type_identifier) @name)",
            "(trait_item name: (type_identifier) @name)",
            "(type_item name: (type_identifier) @name)",
            "(const_item name: (identifier) @name)",
            "(static_item name: (identifier) @name)",
            "(mod_item name: (identifier) @name)",
        ];

        let all_item_queries: Vec<Query> = all_item_query_strs
            .iter()
            .map(|q| compile_query(&language, q, "all-item"))
            .collect::<Result<Vec<_>>>()?;

        let unsafe_query = compile_query(&language, "(unsafe_block) @block", "unsafe")?;

        let trait_impl_queries = vec![
            compile_query(
                &language,
                "(impl_item trait: (type_identifier) @trait type: (type_identifier) @type)",
                "trait_impl",
            )?,
            compile_query(
                &language,
                "(impl_item trait: (scoped_type_identifier) @trait type: (type_identifier) @type)",
                "scoped trait_impl",
            )?,
        ];

        let lifetime_query = compile_query(&language, "(lifetime (identifier) @name)", "lifetime")?;
        let async_query = compile_query(
            &language,
            "(function_item (function_modifiers) @mods)",
            "async",
        )?;
        let derive_query = compile_query(
            &language,
            "(attribute_item (attribute (identifier) @attr_name arguments: (token_tree) @args))",
            "derive",
        )?;
        // ALP-770: match all impl blocks; type extraction done in Rust code
        let impl_query = compile_query(&language, "(impl_item) @impl", "impl")?;

        Ok(Self {
            parser,
            export_queries,
            all_item_queries,
            unsafe_query,
            trait_impl_queries,
            lifetime_query,
            async_query,
            derive_query,
            impl_query,
        })
    }

    fn parse_inner(&mut self, source: &str, binary_crate: bool) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust source"))?;

        let root_node = tree.root_node();

        let mut exports = self.extract_exports(source, root_node, binary_crate);
        let imports = self.extract_imports(source, root_node);

        // ALP-770: extract pub fn from impl blocks of exported types (library crates only).
        // Binary crates use all_item_queries which already capture impl methods as flat entries.
        if !binary_crate {
            let exported_types: HashSet<String> = exports
                .iter()
                .filter(|e| e.parent_class.is_none())
                .map(|e| e.name.clone())
                .collect();
            let methods = self.extract_impl_methods(source, root_node, &exported_types);
            exports.extend(methods);
            exports.sort_by_key(|e| e.start_line);
        }
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
                ..Default::default()
            },
            custom_fields,
        })
    }
}

/// Check if a file path is a Rust binary entry point (main.rs or under a bin/ directory).
fn is_binary_entry_point(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name == "main.rs" {
        return true;
    }
    // src/bin/*.rs files are binary entry points
    path.components().any(|c| c.as_os_str() == "bin")
}

impl Parser for RustParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        self.parse_inner(source, false)
    }

    fn parse_file(&mut self, source: &str, file_path: &Path) -> Result<ParseResult> {
        self.parse_inner(source, is_binary_entry_point(file_path))
    }

    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "rust",
        extensions: &["rs"],
        reexport_filenames: &["mod.rs"],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &["_test.rs"],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };
