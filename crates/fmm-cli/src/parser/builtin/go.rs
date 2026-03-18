use super::query_helpers::{collect_matches_with_lines, compile_query, make_parser};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

pub struct GoParser {
    parser: TSParser,
    func_query: Query,
    type_query: Query,
    const_query: Query,
    var_query: Query,
    import_query: Query,
    /// ALP-796: module name extracted from go.mod (e.g. "github.com/myorg/proj").
    /// None when no go.mod is found — triggers fallback classification heuristic.
    module_name: Option<String>,
}

/// Walk up from `file_path`'s directory looking for `go.mod`. When found,
/// return the module name from the `module` directive.
fn find_go_mod_module(file_path: &Path) -> Option<String> {
    let mut dir = file_path.parent();
    while let Some(d) = dir {
        let go_mod = d.join("go.mod");
        if go_mod.exists()
            && let Ok(content) = std::fs::read_to_string(&go_mod)
        {
            return extract_module_name(&content);
        }
        dir = d.parent();
    }
    None
}

/// Extract the module name from go.mod content.
/// The `module` directive is always the first non-comment, non-empty line.
fn extract_module_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("module") {
            let name = rest.trim();
            if !name.is_empty() {
                // Strip any inline comment.
                let name = name.split_whitespace().next().unwrap_or(name);
                return Some(name.to_string());
            }
        }
        // Any non-empty, non-comment line that isn't `module` means we've
        // passed the preamble without finding it.
        break;
    }
    None
}

impl GoParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_go::LANGUAGE.into();
        let parser = make_parser(&language, "Go")?;

        let func_query = compile_query(
            &language,
            "(source_file (function_declaration name: (identifier) @name))",
            "func",
        )?;
        let type_query = compile_query(
            &language,
            "(source_file (type_declaration (type_spec name: (type_identifier) @name)))",
            "type",
        )?;
        let const_query = compile_query(
            &language,
            "(source_file (const_declaration (const_spec name: (identifier) @name)))",
            "const",
        )?;
        let var_query = compile_query(
            &language,
            "(source_file (var_declaration (var_spec name: (identifier) @name)))",
            "var",
        )?;
        let import_query = compile_query(
            &language,
            "(import_spec path: (interpreted_string_literal) @path)",
            "import",
        )?;

        Ok(Self {
            parser,
            func_query,
            type_query,
            const_query,
            var_query,
            import_query,
            module_name: None,
        })
    }

    fn is_exported(name: &str) -> bool {
        name.starts_with(|c: char| c.is_uppercase())
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let queries = [
            &self.func_query,
            &self.type_query,
            &self.const_query,
            &self.var_query,
        ];

        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        for query in queries {
            for entry in collect_matches_with_lines(query, root_node, source_bytes) {
                if Self::is_exported(&entry.name) && seen.insert(entry.name.clone()) {
                    exports.push(entry);
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
        let mut import_set = HashSet::new();
        let mut dependency_set = HashSet::new();
        let source_bytes = source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes) {
                    let path = text.trim_matches('"');
                    if path.is_empty() {
                        continue;
                    }

                    // ALP-797: three-way classification using the module name from go.mod.
                    //
                    // When the module name is known:
                    //   - Same-module paths (start with module_name + "/") → dependencies
                    //     The module prefix is stripped so dep_matches can resolve them
                    //     against manifest file paths (which are relative to project root).
                    //   - Third-party domain-qualified paths → imports (external)
                    //   - Stdlib paths (no dot in root segment) → imports (external)
                    //
                    // Fallback when go.mod not found: original dot-in-root-segment heuristic.
                    // ALL domain-qualified paths go to dependencies (old behaviour), which
                    // preserves the pre-ALP-795 output for projects without go.mod.
                    let root_pkg = path.split('/').next().unwrap_or(path);
                    if let Some(ref module) = self.module_name {
                        let prefix = format!("{}/", module);
                        if let Some(local_path) = path.strip_prefix(&prefix) {
                            // Same-module import: store the intra-module relative path.
                            dependency_set.insert(local_path.to_string());
                        } else {
                            // Stdlib (no dot) or third-party → external.
                            import_set.insert(path.to_string());
                        }
                    } else if root_pkg.contains('.') {
                        // Fallback: no go.mod — domain-qualified → dependencies (legacy).
                        dependency_set.insert(path.to_string());
                    } else {
                        import_set.insert(path.to_string());
                    }
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

impl Parser for GoParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Go source"))?;

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
                ..Default::default()
            },
            custom_fields: None,
        })
    }

    /// ALP-796: override parse_file() to load the module name from go.mod before parsing.
    fn parse_file(&mut self, source: &str, file_path: &Path) -> Result<ParseResult> {
        self.module_name = find_go_mod_module(file_path);
        self.parse(source)
    }

    fn language_id(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "go",
        extensions: &["go"],
        reexport_filenames: &[],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &["_test.go"],
            filename_prefixes: &[],
            test_symbol_prefixes: &["Test"],
        },
    };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_exported_functions() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

func ExportedFunc() {}
func unexportedFunc() {}
"#;
        let result = parser.parse(source).unwrap();
        assert!(
            result
                .metadata
                .export_names()
                .contains(&"ExportedFunc".to_string())
        );
        assert!(
            !result
                .metadata
                .export_names()
                .contains(&"unexportedFunc".to_string())
        );
    }

    #[test]
    fn parse_go_exported_types() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

type Config struct {
    Name string
}

type handler struct {
    count int
}

type Service interface {
    Start() error
}
"#;
        let result = parser.parse(source).unwrap();
        assert!(
            result
                .metadata
                .export_names()
                .contains(&"Config".to_string())
        );
        assert!(
            result
                .metadata
                .export_names()
                .contains(&"Service".to_string())
        );
        assert!(
            !result
                .metadata
                .export_names()
                .contains(&"handler".to_string())
        );
    }

    #[test]
    fn parse_go_imports() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

import (
    "fmt"
    "os"
    "net/http"
    "github.com/gin-gonic/gin"
)
"#;
        let result = parser.parse(source).unwrap();
        assert!(result.metadata.imports.contains(&"fmt".to_string()));
        assert!(result.metadata.imports.contains(&"os".to_string()));
        assert!(result.metadata.imports.contains(&"net/http".to_string()));
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"github.com/gin-gonic/gin".to_string())
        );
    }

    #[test]
    fn parse_go_constants_and_vars() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

const MaxRetries = 3
const internalLimit = 10

var GlobalState = "init"
var localVar = "hidden"
"#;
        let result = parser.parse(source).unwrap();
        assert!(
            result
                .metadata
                .export_names()
                .contains(&"MaxRetries".to_string())
        );
        assert!(
            result
                .metadata
                .export_names()
                .contains(&"GlobalState".to_string())
        );
        assert!(
            !result
                .metadata
                .export_names()
                .contains(&"internalLimit".to_string())
        );
        assert!(
            !result
                .metadata
                .export_names()
                .contains(&"localVar".to_string())
        );
    }

    #[test]
    fn parse_go_empty() {
        let mut parser = GoParser::new().unwrap();
        let result = parser.parse("").unwrap();
        assert!(result.metadata.exports.is_empty());
        assert!(result.metadata.imports.is_empty());
    }

    // --- ALP-796 / ALP-797: go.mod-aware import classification ---

    fn parse_with_module(source: &str, module: &str) -> ParseResult {
        let mut parser = GoParser::new().unwrap();
        parser.module_name = Some(module.to_string());
        parser.parse(source).unwrap()
    }

    #[test]
    fn same_module_import_classified_as_dependency() {
        let source = r#"
package handler

import "github.com/example/proj/internal/handler"
"#;
        let result = parse_with_module(source, "github.com/example/proj");
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"internal/handler".to_string()),
            "same-module import should be dependency, got: {:?}",
            result.metadata.dependencies
        );
        assert!(
            result.metadata.imports.is_empty(),
            "imports should be empty, got: {:?}",
            result.metadata.imports
        );
    }

    #[test]
    fn third_party_import_classified_as_import() {
        let source = r#"
package main

import "github.com/gin-gonic/gin"
"#;
        let result = parse_with_module(source, "github.com/example/proj");
        assert!(
            result
                .metadata
                .imports
                .contains(&"github.com/gin-gonic/gin".to_string()),
            "third-party import should be in imports, got: {:?}",
            result.metadata.imports
        );
        assert!(
            result.metadata.dependencies.is_empty(),
            "dependencies should be empty, got: {:?}",
            result.metadata.dependencies
        );
    }

    #[test]
    fn stdlib_import_classified_as_import_with_module_name() {
        let source = r#"
package main

import (
    "fmt"
    "net/http"
)
"#;
        let result = parse_with_module(source, "github.com/example/proj");
        assert!(
            result.metadata.imports.contains(&"fmt".to_string()),
            "fmt should be in imports"
        );
        assert!(
            result.metadata.imports.contains(&"net/http".to_string()),
            "net/http should be in imports"
        );
        assert!(result.metadata.dependencies.is_empty());
    }

    #[test]
    fn mixed_imports_with_module_name() {
        let source = r#"
package main

import (
    "fmt"
    "github.com/example/proj/internal/config"
    "github.com/example/proj/pkg/utils"
    "github.com/gin-gonic/gin"
    "golang.org/x/net/context"
)
"#;
        let result = parse_with_module(source, "github.com/example/proj");
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"internal/config".to_string()),
            "internal/config should be a dependency"
        );
        assert!(
            result
                .metadata
                .dependencies
                .contains(&"pkg/utils".to_string()),
            "pkg/utils should be a dependency"
        );
        assert!(
            result.metadata.imports.contains(&"fmt".to_string()),
            "fmt should be in imports"
        );
        assert!(
            result
                .metadata
                .imports
                .contains(&"github.com/gin-gonic/gin".to_string()),
            "gin should be in imports"
        );
        assert!(
            result
                .metadata
                .imports
                .contains(&"golang.org/x/net/context".to_string()),
            "golang.org/x/net/context should be in imports"
        );
    }

    #[test]
    fn extract_module_name_basic() {
        let content = "module github.com/example/myproject\n\ngo 1.21\n";
        assert_eq!(
            extract_module_name(content),
            Some("github.com/example/myproject".to_string())
        );
    }

    #[test]
    fn extract_module_name_with_comment() {
        let content = "// Copyright notice\nmodule github.com/example/proj\n\ngo 1.21\n";
        assert_eq!(
            extract_module_name(content),
            Some("github.com/example/proj".to_string())
        );
    }

    #[test]
    fn extract_module_name_not_found() {
        assert_eq!(extract_module_name("go 1.21\n"), None);
        assert_eq!(extract_module_name(""), None);
    }

    #[test]
    fn find_go_mod_module_reads_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let go_mod = dir.path().join("go.mod");
        let mut f = std::fs::File::create(&go_mod).unwrap();
        writeln!(f, "module github.com/example/myproject").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "go 1.21").unwrap();
        drop(f);

        let source_file = dir.path().join("main.go");
        let result = find_go_mod_module(&source_file);
        assert_eq!(result, Some("github.com/example/myproject".to_string()));
    }
}
