use super::query_helpers::{collect_matches_with_lines, compile_query, make_parser};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashSet;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct PhpParser {
    parser: TSParser,
    class_query: Query,
    interface_query: Query,
    trait_query: Query,
    enum_query: Query,
    func_query: Query,
    const_query: Query,
    public_method_query: Query,
    namespace_use_query: Query,
    require_query: Query,
    namespace_def_query: Query,
    trait_use_query: Query,
}

impl PhpParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_php::LANGUAGE_PHP.into();
        let parser = make_parser(&language, "PHP")?;

        let class_query = compile_query(
            &language,
            "(program (class_declaration name: (name) @name))",
            "class",
        )?;
        let interface_query = compile_query(
            &language,
            "(program (interface_declaration name: (name) @name))",
            "interface",
        )?;
        let trait_query = compile_query(
            &language,
            "(program (trait_declaration name: (name) @name))",
            "trait",
        )?;
        let enum_query = compile_query(
            &language,
            "(program (enum_declaration name: (name) @name))",
            "enum",
        )?;
        let func_query = compile_query(
            &language,
            "(program (function_definition name: (name) @name))",
            "func",
        )?;
        let const_query = compile_query(
            &language,
            "(program (const_declaration (const_element (name) @name)))",
            "const",
        )?;
        // Methods inside class/interface/trait declarations (filtered by visibility in code)
        let public_method_query = compile_query(
            &language,
            "(method_declaration
                (visibility_modifier) @vis
                name: (name) @name)",
            "public method",
        )?;
        // Namespace use declarations (imports)
        let namespace_use_query = compile_query(
            &language,
            "(program (namespace_use_declaration) @decl)",
            "namespace use",
        )?;
        // require/require_once/include/include_once with string paths
        let require_query = compile_query(
            &language,
            r#"[
                (require_expression (string (string_content) @path))
                (require_once_expression (string (string_content) @path))
                (include_expression (string (string_content) @path))
                (include_once_expression (string (string_content) @path))
            ]"#,
            "require",
        )?;
        // Namespace definitions
        let namespace_def_query = compile_query(
            &language,
            "(namespace_definition (namespace_name) @name)",
            "namespace def",
        )?;
        // Trait use inside class bodies (use TraitName;)
        let trait_use_query = compile_query(
            &language,
            "(declaration_list (use_declaration (name) @name))",
            "trait use",
        )?;

        Ok(Self {
            parser,
            class_query,
            interface_query,
            trait_query,
            enum_query,
            func_query,
            const_query,
            public_method_query,
            namespace_use_query,
            require_query,
            namespace_def_query,
            trait_use_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        // Top-level declarations: always exported
        let top_level_queries = [
            &self.class_query,
            &self.interface_query,
            &self.trait_query,
            &self.enum_query,
            &self.func_query,
            &self.const_query,
        ];

        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        for query in top_level_queries {
            for entry in collect_matches_with_lines(query, root_node, source_bytes) {
                if seen.insert(entry.name.clone()) {
                    exports.push(entry);
                }
            }
        }

        // Methods: only public ones
        let capture_names = self.public_method_query.capture_names();
        let vis_idx = capture_names.iter().position(|n| *n == "vis");
        let name_idx = capture_names.iter().position(|n| *n == "name");

        if let (Some(vi), Some(ni)) = (vis_idx, name_idx) {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(&self.public_method_query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                let mut vis_text = "";
                let mut name_text = "";
                let mut name_node = None;
                for capture in m.captures {
                    let idx = capture.index as usize;
                    if idx == vi {
                        vis_text = capture.node.utf8_text(source_bytes).unwrap_or("");
                    } else if idx == ni {
                        name_text = capture.node.utf8_text(source_bytes).unwrap_or("");
                        name_node = Some(capture.node);
                    }
                }
                if vis_text.trim() == "public" {
                    let name = name_text.to_string();
                    if seen.insert(name.clone()) {
                        if let Some(node) = name_node {
                            let decl = node.parent().unwrap_or(node);
                            exports.push(ExportEntry::new(
                                name,
                                decl.start_position().row + 1,
                                decl.end_position().row + 1,
                            ));
                        }
                    }
                }
            }
        }

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    fn extract_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (Vec<String>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut import_set = HashSet::new();
        let mut dependency_set = HashSet::new();

        // Namespace use declarations → imports
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.namespace_use_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    // Extract the full use declaration text and parse namespace paths
                    // The captured node is the full namespace_use_declaration
                    let trimmed = text.trim();
                    // Extract namespace path from "use Foo\Bar\Baz;"
                    if let Some(path) = trimmed.strip_prefix("use ") {
                        let path = path.trim_end_matches(';').trim();
                        // Handle grouped use: "use Foo\{A, B}" → just take "Foo"
                        if let Some(base) = path.split('\\').next() {
                            import_set.insert(base.trim().to_string());
                        }
                    }
                }
            }
        }

        // require/include paths → dependencies (relative) or imports
        let mut cursor2 = QueryCursor::new();
        let mut iter2 = cursor2.matches(&self.require_query, root_node, source_bytes);
        while let Some(m) = iter2.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let path = text.to_string();
                    dependency_set.insert(path);
                }
            }
        }

        let mut imports: Vec<String> = import_set.into_iter().collect();
        let mut dependencies: Vec<String> = dependency_set.into_iter().collect();
        imports.sort();
        dependencies.sort();
        (imports, dependencies)
    }

    fn extract_custom_fields(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Option<std::collections::HashMap<String, serde_json::Value>> {
        let source_bytes = source.as_bytes();
        let mut fields = std::collections::HashMap::new();

        // Namespaces
        let mut namespaces = HashSet::new();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.namespace_def_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    namespaces.insert(text.to_string());
                }
            }
        }
        if !namespaces.is_empty() {
            let mut ns: Vec<String> = namespaces.into_iter().collect();
            ns.sort();
            fields.insert(
                "namespaces".to_string(),
                serde_json::Value::Array(ns.into_iter().map(serde_json::Value::String).collect()),
            );
        }

        // Traits used inside classes
        let mut traits_used = HashSet::new();
        let mut cursor2 = QueryCursor::new();
        let mut iter2 = cursor2.matches(&self.trait_use_query, root_node, source_bytes);
        while let Some(m) = iter2.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    traits_used.insert(text.to_string());
                }
            }
        }
        if !traits_used.is_empty() {
            let mut tu: Vec<String> = traits_used.into_iter().collect();
            tu.sort();
            fields.insert(
                "traits_used".to_string(),
                serde_json::Value::Array(tu.into_iter().map(serde_json::Value::String).collect()),
            );
        }

        if fields.is_empty() {
            None
        } else {
            Some(fields)
        }
    }
}

impl Parser for PhpParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse PHP source"))?;

        let root_node = tree.root_node();
        let exports = self.extract_exports(source, root_node);
        let (imports, dependencies) = self.extract_imports(source, root_node);
        let custom_fields = self.extract_custom_fields(source, root_node);
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
        "php"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "php",
        extensions: &["php"],
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
    fn parse_php_classes() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nclass Foo {}\nclass Bar {}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Foo".to_string()));
        assert!(names.contains(&"Bar".to_string()));
    }

    #[test]
    fn parse_php_interfaces_and_traits() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\ninterface Cacheable { public function cache(); }\ntrait Loggable { public function log() {} }\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Cacheable".to_string()));
        assert!(names.contains(&"Loggable".to_string()));
    }

    #[test]
    fn parse_php_functions_and_constants() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nfunction helper() {}\nconst MAX = 100;\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"helper".to_string()));
        assert!(names.contains(&"MAX".to_string()));
    }

    #[test]
    fn parse_php_public_methods_only() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nclass Foo {\n    public function bar() {}\n    private function baz() {}\n    protected function qux() {}\n}\n";
        let result = parser.parse(source).unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"Foo".to_string()));
        assert!(names.contains(&"bar".to_string()));
        assert!(!names.contains(&"baz".to_string()));
        assert!(!names.contains(&"qux".to_string()));
    }

    #[test]
    fn parse_php_namespace_imports() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nuse App\\Models\\User;\nuse Illuminate\\Http\\Request;\n";
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"App".to_string()));
        assert!(result.metadata.imports.contains(&"Illuminate".to_string()));
    }

    #[test]
    fn parse_php_require_include() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nrequire_once 'vendor/autoload.php';\ninclude './helpers.php';\n";
        let result = parser.parse(source).unwrap();
        assert!(!result.metadata.dependencies.is_empty());
    }

    #[test]
    fn parse_php_namespaces_custom_field() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nnamespace App\\Controllers;\nclass Foo {}\n";
        let result = parser.parse(source).unwrap();
        let fields = result.custom_fields.expect("should have custom fields");
        let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
        assert!(!namespaces.is_empty());
    }

    #[test]
    fn parse_php_empty() {
        let mut parser = PhpParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }

    #[test]
    fn parse_php_enum() {
        let mut parser = PhpParser::new().unwrap();
        let source = "<?php\nenum Status { case Active; case Inactive; }\n";
        let result = parser.parse(source).unwrap();
        assert!(result
            .metadata
            .export_names()
            .contains(&"Status".to_string()));
    }
}
