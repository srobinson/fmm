use super::query_helpers::{compile_query, make_parser};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct CParser {
    parser: TSParser,
    macro_query: Query,
    fn_macro_query: Query,
    system_include_query: Query,
    local_include_query: Query,
}

impl CParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_c::LANGUAGE.into();
        let parser = make_parser(&language, "C")?;

        let macro_query =
            compile_query(&language, "(preproc_def name: (identifier) @name)", "macro")?;
        let fn_macro_query = compile_query(
            &language,
            "(preproc_function_def name: (identifier) @name)",
            "fn macro",
        )?;
        let system_include_query = compile_query(
            &language,
            "(preproc_include path: (system_lib_string) @path)",
            "system include",
        )?;
        let local_include_query = compile_query(
            &language,
            "(preproc_include path: (string_literal) @path)",
            "local include",
        )?;

        Ok(Self {
            parser,
            macro_query,
            fn_macro_query,
            system_include_query,
            local_include_query,
        })
    }

    /// Recursively unwrap declarator chains to find the leaf identifier.
    /// Handles: function_declarator, pointer_declarator, parenthesized_declarator, array_declarator.
    fn unwrap_declarator(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        match node.kind() {
            "identifier" | "type_identifier" | "primitive_type" => {
                node.utf8_text(source_bytes).ok().map(|s| s.to_string())
            }
            "function_declarator" | "pointer_declarator" | "array_declarator" => {
                let inner = node.child_by_field_name("declarator")?;
                Self::unwrap_declarator(&inner, source_bytes)
            }
            "parenthesized_declarator" => {
                // May not have a "declarator" field — search named children
                if let Some(inner) = node.child_by_field_name("declarator") {
                    return Self::unwrap_declarator(&inner, source_bytes);
                }
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if let Some(name) = Self::unwrap_declarator(&child, source_bytes) {
                        return Some(name);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Check if a function_definition node has a `static` storage class specifier.
    fn is_static(node: &tree_sitter::Node, source_bytes: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "storage_class_specifier" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    if text == "static" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract a named struct/enum type from a declaration node.
    /// Only matches definitions (with body), not references or forward declarations.
    fn extract_type_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "struct_specifier" | "enum_specifier" => {
                    if child.child_by_field_name("body").is_some() {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            return name_node
                                .utf8_text(source_bytes)
                                .ok()
                                .map(|s| s.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_exports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<ExportEntry>, Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut macros = Vec::new();
        let mut typedefs = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if !Self::is_static(&child, source_bytes) {
                        if let Some(declarator) = child.child_by_field_name("declarator") {
                            if let Some(name) = Self::unwrap_declarator(&declarator, source_bytes) {
                                if seen.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                    }
                }
                "declaration" => {
                    // Struct/enum definitions at file scope (when combined with variable decl)
                    if let Some(name) = Self::extract_type_name(&child, source_bytes) {
                        if seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                "struct_specifier" | "enum_specifier" => {
                    // Standalone type definitions (direct root children)
                    if child.child_by_field_name("body").is_some() {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            if let Ok(name) = name_node.utf8_text(source_bytes) {
                                let name = name.to_string();
                                if seen.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                    }
                }
                "type_definition" => {
                    // Typedef alias name (the declarator)
                    if let Some(declarator) = child.child_by_field_name("declarator") {
                        if let Some(name) = Self::unwrap_declarator(&declarator, source_bytes) {
                            typedefs.push(name.clone());
                            if seen.insert(name.clone()) {
                                exports.push(ExportEntry::new(
                                    name,
                                    child.start_position().row + 1,
                                    child.end_position().row + 1,
                                ));
                            }
                        }
                    }
                    // Named struct/enum inside typedef (e.g., typedef struct Foo { ... } Foo)
                    if let Some(name) = Self::extract_type_name(&child, source_bytes) {
                        if seen.insert(name.clone()) {
                            exports.push(ExportEntry::new(
                                name,
                                child.start_position().row + 1,
                                child.end_position().row + 1,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Macros via queries (handles #define inside #ifdef blocks too)
        for query in [&self.macro_query, &self.fn_macro_query] {
            let mut qcursor = QueryCursor::new();
            let mut iter = qcursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let name = text.to_string();
                        macros.push(name.clone());
                        if seen.insert(name.clone()) {
                            let parent = capture.node.parent().unwrap_or(capture.node);
                            exports.push(ExportEntry::new(
                                name,
                                parent.start_position().row + 1,
                                parent.end_position().row + 1,
                            ));
                        }
                    }
                }
            }
        }

        exports.sort_by_key(|e| e.start_line);
        macros.sort();
        macros.dedup();
        typedefs.sort();
        (exports, macros, typedefs)
    }

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut dependency_set = HashSet::new();

        // System includes → imports
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.system_include_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    import_set.insert(
                        text.trim_start_matches('<')
                            .trim_end_matches('>')
                            .to_string(),
                    );
                }
            }
        }

        // Local includes → dependencies
        let mut cursor2 = QueryCursor::new();
        let mut iter2 = cursor2.matches(&self.local_include_query, root_node, source_bytes);
        while let Some(m) = iter2.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    dependency_set.insert(text.trim_matches('"').to_string());
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        let mut dependencies: Vec<String> = dependency_set.into_iter().collect();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }
}

impl Parser for CParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse C source"))?;

        let root_node = tree.root_node();
        let (exports, macros, typedefs) = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let loc = source.lines().count();

        let mut custom_fields = HashMap::new();
        if !macros.is_empty() {
            custom_fields.insert(
                "macros".to_string(),
                serde_json::Value::Array(
                    macros.into_iter().map(serde_json::Value::String).collect(),
                ),
            );
        }
        if !typedefs.is_empty() {
            custom_fields.insert(
                "typedefs".to_string(),
                serde_json::Value::Array(
                    typedefs
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
                ..Default::default()
            },
            custom_fields: if custom_fields.is_empty() {
                None
            } else {
                Some(custom_fields)
            },
        })
    }

    fn language_id(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "c",
        extensions: &["c", "h"],
        reexport_filenames: &[],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &[],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        },
    };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_c_functions() {
        let mut parser = CParser::new().unwrap();
        let source = "int main() { return 0; }\nvoid process() {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"main".to_string()));
        assert!(names.contains(&"process".to_string()));
    }

    #[test]
    fn parse_c_static_excluded() {
        let mut parser = CParser::new().unwrap();
        let source = "static int helper() { return 0; }\nint public_fn() { return 1; }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"public_fn".to_string()));
        assert!(!names.contains(&"helper".to_string()));
    }

    #[test]
    fn parse_c_pointer_return() {
        let mut parser = CParser::new().unwrap();
        let source = "char *get_name() { return \"hello\"; }\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"get_name".to_string()));
    }

    #[test]
    fn parse_c_structs_and_enums() {
        let mut parser = CParser::new().unwrap();
        let source = "struct Point { int x; int y; };\nenum Color { RED, GREEN, BLUE };\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Point".to_string()));
        assert!(names.contains(&"Color".to_string()));
    }

    #[test]
    fn parse_c_typedefs() {
        let mut parser = CParser::new().unwrap();
        let source = "typedef unsigned long size_t;\ntypedef int (*Callback)(void *);\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"size_t".to_string()));
        assert!(names.contains(&"Callback".to_string()));
    }

    #[test]
    fn parse_c_macros() {
        let mut parser = CParser::new().unwrap();
        let source = "#define MAX 100\n#define MIN(a, b) ((a) < (b) ? (a) : (b))\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"MAX".to_string()));
        assert!(names.contains(&"MIN".to_string()));
        let fields = result.custom_fields.unwrap();
        let macros = fields.get("macros").unwrap().as_array().unwrap();
        assert_eq!(macros.len(), 2);
    }

    #[test]
    fn parse_c_includes() {
        let mut parser = CParser::new().unwrap();
        let source = "#include <stdio.h>\n#include <stdlib.h>\n#include \"config.h\"\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
        assert!(result.metadata.imports.contains(&"stdlib.h".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"config.h".to_string()));
    }

    #[test]
    fn parse_c_empty() {
        let mut parser = CParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
