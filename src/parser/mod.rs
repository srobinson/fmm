pub mod builtin;
pub mod plugin;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
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

    /// The language identifier used in frontmatter sections (e.g., "rust", "python").
    fn language_id(&self) -> &'static str;

    /// File extensions this parser handles.
    fn extensions(&self) -> &'static [&'static str];
}

type ParserFactory = Box<dyn Fn() -> Result<Box<dyn Parser>> + Send + Sync>;

/// Registry that maps file extensions to parser constructors.
pub struct ParserRegistry {
    factories: HashMap<String, ParserFactory>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
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
        // TypeScript / JavaScript
        self.register(&["ts", "tsx", "js", "jsx"], || {
            Ok(Box::new(builtin::typescript::TypeScriptParser::new()?))
        });

        // Python
        self.register(&["py"], || {
            Ok(Box::new(builtin::python::PythonParser::new()?))
        });

        // Rust
        self.register(&["rs"], || Ok(Box::new(builtin::rust::RustParser::new()?)));

        // Go
        self.register(&["go"], || Ok(Box::new(builtin::go::GoParser::new()?)));

        // Java
        self.register(&["java"], || {
            Ok(Box::new(builtin::java::JavaParser::new()?))
        });

        // C++
        self.register(&["cpp", "hpp", "cc", "hh", "cxx", "hxx"], || {
            Ok(Box::new(builtin::cpp::CppParser::new()?))
        });

        // C#
        self.register(&["cs"], || {
            Ok(Box::new(builtin::csharp::CSharpParser::new()?))
        });

        // Ruby
        self.register(&["rb"], || Ok(Box::new(builtin::ruby::RubyParser::new()?)));
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
    }

    #[test]
    fn registry_returns_error_for_unknown_extension() {
        let registry = ParserRegistry::with_builtins();
        assert!(registry.get_parser("zig").is_err());
    }

    #[test]
    fn registry_creates_working_typescript_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("ts").unwrap();
        let result = parser.parse("export function hello() {}").unwrap();
        assert_eq!(result.metadata.exports, vec!["hello"]);
    }

    #[test]
    fn registry_creates_working_python_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("py").unwrap();
        let result = parser
            .parse("def hello():\n    pass\n\ndef world():\n    pass")
            .unwrap();
        assert!(result.metadata.exports.contains(&"hello".to_string()));
        assert!(result.metadata.exports.contains(&"world".to_string()));
    }

    #[test]
    fn registry_creates_working_rust_parser() {
        let registry = ParserRegistry::with_builtins();
        let mut parser = registry.get_parser("rs").unwrap();
        let result = parser.parse("pub fn hello() {}").unwrap();
        assert_eq!(result.metadata.exports, vec!["hello"]);
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
