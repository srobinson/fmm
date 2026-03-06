use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Language, Parser as TSParser};

use super::query_helpers::make_parser;

pub struct ElixirParser {
    parser: TSParser,
}

impl ElixirParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_elixir::LANGUAGE.into();
        let parser = make_parser(&language, "Elixir")?;
        Ok(Self { parser })
    }

    fn get_call_target<'a>(node: &tree_sitter::Node, source_bytes: &'a [u8]) -> Option<&'a str> {
        let target = node.child_by_field_name("target")?;
        target.utf8_text(source_bytes).ok()
    }

    fn get_module_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                let mut arg_cursor = child.walk();
                for arg in child.children(&mut arg_cursor) {
                    if arg.kind() == "alias" {
                        return arg.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                }
            }
        }
        None
    }

    fn get_function_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                let mut arg_cursor = child.walk();
                for arg in child.children(&mut arg_cursor) {
                    if arg.kind() == "call" {
                        let target = arg.child_by_field_name("target")?;
                        return target.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                    if arg.kind() == "binary_operator" {
                        let mut bin_cursor = arg.walk();
                        for bin_child in arg.children(&mut bin_cursor) {
                            if bin_child.kind() == "call" {
                                let target = bin_child.child_by_field_name("target")?;
                                return target.utf8_text(source_bytes).ok().map(|s| s.to_string());
                            }
                        }
                    }
                    if arg.kind() == "identifier" {
                        return arg.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                }
            }
        }
        None
    }

    fn find_do_block<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        let result = node
            .children(&mut cursor)
            .find(|child| child.kind() == "do_block");
        result
    }

    fn get_import_module_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                let mut arg_cursor = child.walk();
                for arg in child.children(&mut arg_cursor) {
                    if arg.kind() == "alias" {
                        return arg.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                    if arg.kind() == "dot" {
                        return arg.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                    if arg.kind() == "identifier" {
                        return arg.utf8_text(source_bytes).ok().map(|s| s.to_string());
                    }
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_body(
        node: tree_sitter::Node,
        source_bytes: &[u8],
        seen_exports: &mut HashSet<String>,
        exports: &mut Vec<ExportEntry>,
        import_set: &mut HashSet<String>,
        macro_count: &mut u64,
        protocol_count: &mut u64,
        behaviour_count: &mut u64,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call" {
                if let Some(target) = Self::get_call_target(&child, source_bytes) {
                    match target {
                        "defmodule" => {
                            if let Some(name) = Self::get_module_name(&child, source_bytes) {
                                if seen_exports.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                            if let Some(do_block) = Self::find_do_block(&child) {
                                Self::walk_body(
                                    do_block,
                                    source_bytes,
                                    seen_exports,
                                    exports,
                                    import_set,
                                    macro_count,
                                    protocol_count,
                                    behaviour_count,
                                );
                            }
                        }
                        "defprotocol" => {
                            *protocol_count += 1;
                            if let Some(name) = Self::get_module_name(&child, source_bytes) {
                                if seen_exports.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                            if let Some(do_block) = Self::find_do_block(&child) {
                                Self::walk_body(
                                    do_block,
                                    source_bytes,
                                    seen_exports,
                                    exports,
                                    import_set,
                                    macro_count,
                                    protocol_count,
                                    behaviour_count,
                                );
                            }
                        }
                        "def" | "defdelegate" => {
                            if let Some(name) = Self::get_function_name(&child, source_bytes) {
                                if seen_exports.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                        "defmacro" => {
                            *macro_count += 1;
                            if let Some(name) = Self::get_function_name(&child, source_bytes) {
                                if seen_exports.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                        "defguard" => {
                            if let Some(name) = Self::get_function_name(&child, source_bytes) {
                                if seen_exports.insert(name.clone()) {
                                    exports.push(ExportEntry::new(
                                        name,
                                        child.start_position().row + 1,
                                        child.end_position().row + 1,
                                    ));
                                }
                            }
                        }
                        "use" | "import" | "alias" | "require" => {
                            if let Some(module) = Self::get_import_module_name(&child, source_bytes)
                            {
                                let root = module.split('.').next().unwrap_or(&module);
                                import_set.insert(root.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }

            if child.kind() == "unary_operator" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    if text.contains("@behaviour") || text.contains("@behavior") {
                        *behaviour_count += 1;
                    }
                }
            }

            if child.kind() == "do_block" {
                Self::walk_body(
                    child,
                    source_bytes,
                    seen_exports,
                    exports,
                    import_set,
                    macro_count,
                    protocol_count,
                    behaviour_count,
                );
            }
        }
    }
}

impl Parser for ElixirParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Elixir source"))?;

        let root_node = tree.root_node();
        let source_bytes = source.as_bytes();
        let mut seen_exports = HashSet::new();
        let mut exports = Vec::new();
        let mut import_set = HashSet::new();
        let mut macro_count: u64 = 0;
        let mut protocol_count: u64 = 0;
        let mut behaviour_count: u64 = 0;

        Self::walk_body(
            root_node,
            source_bytes,
            &mut seen_exports,
            &mut exports,
            &mut import_set,
            &mut macro_count,
            &mut protocol_count,
            &mut behaviour_count,
        );

        exports.sort_by_key(|e| e.start_line);

        let mut imports: Vec<String> = import_set.into_iter().collect();
        imports.sort();
        let dependencies: Vec<String> = Vec::new();

        let mut fields = HashMap::new();
        if macro_count > 0 {
            fields.insert(
                "macros".to_string(),
                serde_json::Value::Number(macro_count.into()),
            );
        }
        if protocol_count > 0 {
            fields.insert(
                "protocols".to_string(),
                serde_json::Value::Number(protocol_count.into()),
            );
        }
        if behaviour_count > 0 {
            fields.insert(
                "behaviours".to_string(),
                serde_json::Value::Number(behaviour_count.into()),
            );
        }
        let custom_fields = if fields.is_empty() {
            None
        } else {
            Some(fields)
        };

        let loc = source.lines().count();

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

    fn language_id(&self) -> &'static str {
        "elixir"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defmodule() {
        let mut parser = ElixirParser::new().unwrap();
        let source = "defmodule MyApp do\nend\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert_eq!(names, vec!["MyApp"]);
    }

    #[test]
    fn parse_def_and_defp() {
        let mut parser = ElixirParser::new().unwrap();
        let source = "defmodule M do\n  def public_fn() do\n    :ok\n  end\n  defp private_fn() do\n    :private\n  end\nend\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"M".to_string()));
        assert!(names.contains(&"public_fn".to_string()));
        assert!(!names.contains(&"private_fn".to_string()));
    }

    #[test]
    fn parse_defprotocol() {
        let mut parser = ElixirParser::new().unwrap();
        let source = "defprotocol Printable do\n  def print(value)\nend\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Printable".to_string()));
        assert!(names.contains(&"print".to_string()));
    }

    #[test]
    fn parse_defmacro() {
        let mut parser = ElixirParser::new().unwrap();
        let source = "defmodule M do\n  defmacro my_macro() do\n    quote do: :ok\n  end\nend\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"my_macro".to_string()));
        let fields = result.custom_fields.unwrap();
        assert_eq!(fields.get("macros").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn parse_imports() {
        let mut parser = ElixirParser::new().unwrap();
        let source =
            "defmodule M do\n  use Plug\n  import Logger\n  alias MyApp\n  require EEx\nend\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"Plug".to_string()));
        assert!(result.metadata.imports.contains(&"Logger".to_string()));
        assert!(result.metadata.imports.contains(&"MyApp".to_string()));
        assert!(result.metadata.imports.contains(&"EEx".to_string()));
    }

    #[test]
    fn parse_empty() {
        let mut parser = ElixirParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
        assert_eq!(result.metadata.loc, 0);
    }

    #[test]
    fn exports_sorted_by_line() {
        let mut parser = ElixirParser::new().unwrap();
        let source = "defmodule Z do\n  def zebra(), do: :ok\n  def alpha(), do: :ok\nend\n";
        let result = parser.parse(source).unwrap();
        let lines: Vec<usize> = result
            .metadata
            .exports
            .iter()
            .map(|e| e.start_line)
            .collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
    }
}
