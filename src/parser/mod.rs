pub mod builtin;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A single exported symbol with its source location (1-indexed lines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportEntry {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    /// When set, this entry is a method of the named class, not a top-level export.
    /// The method renders under `methods:` in the sidecar as `ClassName.method: [start, end]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_class: Option<String>,
}

impl ExportEntry {
    pub fn new(name: String, start_line: usize, end_line: usize) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: None,
        }
    }

    /// Create a method entry belonging to a parent class.
    pub fn method(name: String, start_line: usize, end_line: usize, parent_class: String) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: Some(parent_class),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub exports: Vec<ExportEntry>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
    /// Named imports per source module (TS/JS only).
    /// Key = import path as written in source (`"./ReactFiberWorkLoop"`).
    /// Value = original exported names (alias-resolved: store `foo`, not `bar`, for `import { foo as bar }`).
    /// Also captures named re-exports (`export { foo } from './mod'`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub named_imports: HashMap<String, Vec<String>>,
    /// Source paths of namespace imports (`import * as X from '...'`) and
    /// wildcard re-exports (`export * from '...'`). Stored as written in source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespace_imports: Vec<String>,
}

impl Metadata {
    /// Convenience: get top-level export names as strings.
    /// Excludes method entries (those with `parent_class` set).
    pub fn export_names(&self) -> Vec<String> {
        self.exports
            .iter()
            .filter(|e| e.parent_class.is_none())
            .map(|e| e.name.clone())
            .collect()
    }
}

/// Result of parsing a source file: metadata plus optional language-specific fields.
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub metadata: Metadata,
    pub custom_fields: Option<HashMap<String, serde_json::Value>>,
}

pub trait Parser: Send + Sync {
    /// Parse source in a single tree-sitter pass, returning metadata and custom fields together.
    fn parse(&mut self, source: &str) -> Result<ParseResult>;

    /// Parse with file path context. Override for language-specific behavior
    /// based on file location (e.g., Rust binary crate detection).
    fn parse_file(&mut self, source: &str, _file_path: &Path) -> Result<ParseResult> {
        self.parse(source)
    }

    /// The language identifier used in frontmatter sections (e.g., "rust", "python").
    fn language_id(&self) -> &'static str;

    /// File extensions this parser handles.
    fn extensions(&self) -> &'static [&'static str];
}

type ParserFactory = Box<dyn Fn() -> Result<Box<dyn Parser>> + Send + Sync>;

/// Registry that maps file extensions to parser constructors.
pub struct ParserRegistry {
    factories: HashMap<String, ParserFactory>,
    language_ids: HashMap<String, &'static str>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            language_ids: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all builtin parsers.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtin();
        registry
    }

    /// Register a parser factory for a set of extensions.
    pub fn register<F>(&mut self, extensions: &[&str], factory: F)
    where
        F: Fn() -> Result<Box<dyn Parser>> + Send + Sync + 'static,
    {
        let factory = Box::new(factory);
        // Share the factory via Arc for multiple extensions
        let factory = std::sync::Arc::new(factory);
        for ext in extensions {
            let f = factory.clone();
            self.factories
                .insert(ext.to_string(), Box::new(move || f()));
        }
    }

    /// Register all builtin parsers.
    fn register_builtin(&mut self) {
        // TypeScript / JavaScript (ALP-753: split TS and TSX into separate parsers)
        self.register(&["ts", "js"], || {
            Ok(Box::new(builtin::typescript::TypeScriptParser::new()?))
        });
        self.register_language_id(&["ts", "js"], "typescript");

        // TSX / JSX — uses LANGUAGE_TSX grammar for correct JSX angle-bracket parsing
        self.register(&["tsx", "jsx"], || {
            Ok(Box::new(builtin::typescript::TypeScriptParser::new_tsx()?))
        });
        self.register_language_id(&["tsx", "jsx"], "tsx");

        // Python
        self.register(&["py"], || {
            Ok(Box::new(builtin::python::PythonParser::new()?))
        });
        self.register_language_id(&["py"], "python");

        // Rust
        self.register(&["rs"], || Ok(Box::new(builtin::rust::RustParser::new()?)));
        self.register_language_id(&["rs"], "rust");

        // Go
        self.register(&["go"], || Ok(Box::new(builtin::go::GoParser::new()?)));
        self.register_language_id(&["go"], "go");

        // Java
        self.register(&["java"], || {
            Ok(Box::new(builtin::java::JavaParser::new()?))
        });
        self.register_language_id(&["java"], "java");

        // C++
        self.register(&["cpp", "hpp", "cc", "hh", "cxx", "hxx"], || {
            Ok(Box::new(builtin::cpp::CppParser::new()?))
        });
        self.register_language_id(&["cpp", "hpp", "cc", "hh", "cxx", "hxx"], "cpp");

        // C#
        self.register(&["cs"], || {
            Ok(Box::new(builtin::csharp::CSharpParser::new()?))
        });
        self.register_language_id(&["cs"], "csharp");

        // Ruby
        self.register(&["rb"], || Ok(Box::new(builtin::ruby::RubyParser::new()?)));
        self.register_language_id(&["rb"], "ruby");

        // PHP
        self.register(&["php"], || Ok(Box::new(builtin::php::PhpParser::new()?)));
        self.register_language_id(&["php"], "php");

        // C
        self.register(&["c", "h"], || Ok(Box::new(builtin::c::CParser::new()?)));
        self.register_language_id(&["c", "h"], "c");

        // Zig
        self.register(&["zig"], || Ok(Box::new(builtin::zig::ZigParser::new()?)));
        self.register_language_id(&["zig"], "zig");

        // Lua
        self.register(&["lua"], || Ok(Box::new(builtin::lua::LuaParser::new()?)));
        self.register_language_id(&["lua"], "lua");

        // Scala
        self.register(&["scala", "sc"], || {
            Ok(Box::new(builtin::scala::ScalaParser::new()?))
        });
        self.register_language_id(&["scala", "sc"], "scala");

        // Swift
        self.register(&["swift"], || {
            Ok(Box::new(builtin::swift::SwiftParser::new()?))
        });
        self.register_language_id(&["swift"], "swift");

        // Kotlin
        self.register(&["kt", "kts"], || {
            Ok(Box::new(builtin::kotlin::KotlinParser::new()?))
        });
        self.register_language_id(&["kt", "kts"], "kotlin");

        // Dart
        self.register(&["dart"], || {
            Ok(Box::new(builtin::dart::DartParser::new()?))
        });
        self.register_language_id(&["dart"], "dart");

        // Elixir
        self.register(&["ex", "exs"], || {
            Ok(Box::new(builtin::elixir::ElixirParser::new()?))
        });
        self.register_language_id(&["ex", "exs"], "elixir");
    }

    fn register_language_id(&mut self, extensions: &[&str], language_id: &'static str) {
        for ext in extensions {
            self.language_ids.insert(ext.to_string(), language_id);
        }
    }

    /// Get the language ID for a file extension without constructing a parser.
    pub fn language_id_for(&self, extension: &str) -> Option<&'static str> {
        self.language_ids.get(extension).copied()
    }

    /// Get a new parser instance for the given file extension.
    pub fn get_parser(&self, extension: &str) -> Result<Box<dyn Parser>> {
        let factory = self
            .factories
            .get(extension)
            .ok_or_else(|| anyhow::anyhow!("No parser registered for extension: .{}", extension))?;
        factory()
    }

    /// Check if a parser exists for the given extension.
    #[cfg(test)]
    pub fn has_parser(&self, extension: &str) -> bool {
        self.factories.contains_key(extension)
    }

    /// List all registered extensions.
    #[cfg(test)]
    pub fn extensions(&self) -> Vec<&str> {
        let mut exts: Vec<&str> = self.factories.keys().map(|s| s.as_str()).collect();
        exts.sort();
        exts
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_builtin_parsers() {
        let registry = ParserRegistry::with_builtins();
        assert!(registry.has_parser("ts"));
        assert!(registry.has_parser("tsx"));
        assert!(registry.has_parser("js"));
        assert!(registry.has_parser("jsx"));
        assert!(registry.has_parser("py"));
        assert!(registry.has_parser("rs"));
        assert!(registry.has_parser("go"));
        assert!(registry.has_parser("java"));
        assert!(registry.has_parser("cpp"));
        assert!(registry.has_parser("hpp"));
        assert!(registry.has_parser("cs"));
        assert!(registry.has_parser("rb"));
        assert!(registry.has_parser("php"));
        assert!(registry.has_parser("c"));
        assert!(registry.has_parser("h"));
        assert!(registry.has_parser("zig"));
        assert!(registry.has_parser("lua"));
        assert!(registry.has_parser("scala"));
        assert!(registry.has_parser("sc"));
        assert!(registry.has_parser("swift"));
        assert!(registry.has_parser("kt"));
        assert!(registry.has_parser("kts"));
        assert!(registry.has_parser("dart"));
        assert!(registry.has_parser("ex"));
        assert!(registry.has_parser("exs"));
    }

    #[test]
    fn language_id_for_avoids_parser_construction() {
        let registry = ParserRegistry::with_builtins();
        assert_eq!(registry.language_id_for("rs"), Some("rust"));
        assert_eq!(registry.language_id_for("ts"), Some("typescript"));
        assert_eq!(registry.language_id_for("js"), Some("typescript"));
        assert_eq!(registry.language_id_for("tsx"), Some("tsx"));
        assert_eq!(registry.language_id_for("jsx"), Some("tsx"));
        assert_eq!(registry.language_id_for("py"), Some("python"));
        assert_eq!(registry.language_id_for("go"), Some("go"));
        assert_eq!(registry.language_id_for("java"), Some("java"));
        assert_eq!(registry.language_id_for("cpp"), Some("cpp"));
        assert_eq!(registry.language_id_for("cs"), Some("csharp"));
        assert_eq!(registry.language_id_for("rb"), Some("ruby"));
        assert_eq!(registry.language_id_for("php"), Some("php"));
        assert_eq!(registry.language_id_for("c"), Some("c"));
        assert_eq!(registry.language_id_for("h"), Some("c"));
        assert_eq!(registry.language_id_for("zig"), Some("zig"));
        assert_eq!(registry.language_id_for("lua"), Some("lua"));
        assert_eq!(registry.language_id_for("scala"), Some("scala"));
        assert_eq!(registry.language_id_for("sc"), Some("scala"));
        assert_eq!(registry.language_id_for("swift"), Some("swift"));
        assert_eq!(registry.language_id_for("kt"), Some("kotlin"));
        assert_eq!(registry.language_id_for("kts"), Some("kotlin"));
        assert_eq!(registry.language_id_for("dart"), Some("dart"));
        assert_eq!(registry.language_id_for("ex"), Some("elixir"));
        assert_eq!(registry.language_id_for("exs"), Some("elixir"));
    }

    #[test]
    fn registry_returns_error_for_unknown_extension() {
        let registry = ParserRegistry::with_builtins();
        assert!(registry.get_parser("unknown_ext").is_err());
    }

    #[test]
    fn registry_creates_working_typescript_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("ts").unwrap();
        let result = parser.parse("export function hello() {}").unwrap();
        assert_eq!(result.metadata.export_names(), vec!["hello"]);
    }

    #[test]
    fn registry_creates_working_python_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("py").unwrap();
        let result = parser
            .parse("def hello():\n    pass\n\ndef world():\n    pass")
            .unwrap();
        let names = result.metadata.export_names();
        assert!(names.contains(&"hello".to_string()));
        assert!(names.contains(&"world".to_string()));
    }

    #[test]
    fn registry_creates_working_rust_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("rs").unwrap();
        let result = parser.parse("pub fn hello() {}").unwrap();
        assert_eq!(result.metadata.export_names(), vec!["hello"]);
    }

    #[test]
    fn registry_lists_extensions() {
        let registry = ParserRegistry::with_builtins();
        let exts = registry.extensions();
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"py"));
        assert!(exts.contains(&"rs"));
    }

    #[test]
    fn registry_custom_register() {
        let mut registry = ParserRegistry::new();
        registry.register(&["custom"], || {
            Ok(Box::new(builtin::typescript::TypeScriptParser::new()?))
        });
        assert!(registry.has_parser("custom"));
        assert!(!registry.has_parser("ts"));
    }

    #[test]
    fn default_registry_has_builtins() {
        let registry = ParserRegistry::default();
        assert!(registry.has_parser("ts"));
        assert!(registry.has_parser("py"));
        assert!(registry.has_parser("rs"));
    }
}
