use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashSet;
use tree_sitter::{Language, Parser as TSParser};

pub struct LuaParser {
    parser: TSParser,
}

impl LuaParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_lua::LANGUAGE.into();
        let mut parser = TSParser::new();
        parser
            .set_language(&language)
            .map_err(|e| anyhow::anyhow!("Failed to set Lua language: {}", e))?;

        Ok(Self { parser })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            if child.kind() == "function_declaration" {
                let has_local = Self::has_local_keyword(&child);

                if has_local {
                    // `local function name()` — private, skip
                    continue;
                }

                // Look for the function name: either an identifier (global) or dot_index_expression (M.name)
                let mut inner_cursor = child.walk();
                for fn_child in child.children(&mut inner_cursor) {
                    match fn_child.kind() {
                        "identifier" => {
                            // `function globalFunc()` — global, exported
                            if let Ok(name) = fn_child.utf8_text(source_bytes) {
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
                        "dot_index_expression" => {
                            // `function M.process()` — module method, export the method name
                            if let Some(method_name) =
                                Self::extract_dot_method_name(&fn_child, source_bytes)
                            {
                                if seen.insert(method_name.clone()) {
                                    exports.push(ExportEntry::new(
                                        method_name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    /// Check if a function_declaration has a `local` keyword.
    fn has_local_keyword(node: &tree_sitter::Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "local" {
                return true;
            }
            // local comes before function keyword
            if child.kind() == "function" {
                break;
            }
        }
        false
    }

    /// Extract the method name from a dot_index_expression (e.g., "process" from "M.process").
    fn extract_dot_method_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        // dot_index_expression has children: identifier "M", ".", identifier "process"
        // The last identifier after the dot is the method name
        let mut last_ident = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                last_ident = child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
        last_ident
    }

    /// Extract require() calls from the entire tree.
    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut dependency_set = HashSet::new();

        Self::walk_requires(
            root_node,
            source_bytes,
            &mut import_set,
            &mut dependency_set,
        );

        let mut imports: Vec<String> = import_set.into_iter().collect();
        let mut dependencies: Vec<String> = dependency_set.into_iter().collect();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }

    fn walk_requires(
        node: tree_sitter::Node,
        source_bytes: &[u8],
        imports: &mut HashSet<String>,
        dependencies: &mut HashSet<String>,
    ) {
        if node.kind() == "function_call" {
            // Check if this is a require() call
            let mut cursor = node.walk();
            let mut is_require = false;
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Ok(text) = child.utf8_text(source_bytes) {
                        if text == "require" {
                            is_require = true;
                        }
                    }
                }
                if is_require && child.kind() == "arguments" {
                    Self::extract_require_arg(&child, source_bytes, imports, dependencies);
                }
            }
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_requires(child, source_bytes, imports, dependencies);
        }
    }

    fn extract_require_arg(
        args_node: &tree_sitter::Node,
        source_bytes: &[u8],
        imports: &mut HashSet<String>,
        dependencies: &mut HashSet<String>,
    ) {
        let mut cursor = args_node.walk();
        for child in args_node.children(&mut cursor) {
            if child.kind() == "string" {
                // Find string_content child
                let mut str_cursor = child.walk();
                for str_child in child.children(&mut str_cursor) {
                    if str_child.kind() == "string_content" {
                        if let Ok(path) = str_child.utf8_text(source_bytes) {
                            if !path.is_empty() {
                                if path.starts_with('.') {
                                    dependencies.insert(path.to_string());
                                } else {
                                    imports.insert(path.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Parser for LuaParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Lua source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let loc = source.lines().count();

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
            },
            custom_fields: None,
        })
    }

    fn language_id(&self) -> &'static str {
        "lua"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lua_module_functions() {
        let mut parser = LuaParser::new().unwrap();
        let source = "local M = {}\nfunction M.process(data) return data end\nfunction M.transform(x) return x end\nreturn M\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"process".to_string()));
        assert!(names.contains(&"transform".to_string()));
    }

    #[test]
    fn parse_lua_global_functions() {
        let mut parser = LuaParser::new().unwrap();
        let source = "function globalFunc() return 42 end\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"globalFunc".to_string()));
    }

    #[test]
    fn parse_lua_local_excluded() {
        let mut parser = LuaParser::new().unwrap();
        let source = "local function helper() return true end\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.exports.is_empty());
    }

    #[test]
    fn parse_lua_requires() {
        let mut parser = LuaParser::new().unwrap();
        let source = "local json = require(\"json\")\nlocal utils = require(\"./utils\")\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"json".to_string()));
        assert!(result
            .metadata
            .dependencies
            .contains(&"./utils".to_string()));
    }

    #[test]
    fn parse_lua_empty() {
        let mut parser = LuaParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }
}
