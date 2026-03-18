pub mod builtin;

use anyhow::Result;

/// Language-specific test file naming conventions.
///
/// These supplement the configurable `test_patterns` in `.fmmrc.toml`.
/// Language parsers provide these patterns so downstream subsystems (glossary,
/// file listing) can classify test files without knowing each language's
/// conventions.
#[derive(Debug, Clone, Default)]
pub struct LanguageTestPatterns {
    /// Filename suffixes that indicate a test file (e.g. `"_test.go"`).
    pub filename_suffixes: &'static [&'static str],
    /// Filename prefixes that indicate a test file (e.g. `"test_"`).
    pub filename_prefixes: &'static [&'static str],
    /// Symbol name prefixes that indicate a test export (e.g. `"test_"`, `"Test"`).
    pub test_symbol_prefixes: &'static [&'static str],
}

/// Register a language parser, its language-id mapping, and its descriptor in one atomic call.
///
/// Guarantees that `register()`, `register_language_id()`, and `register_descriptor()` are
/// always called together. It is impossible to add a language via this macro without also
/// capturing its descriptor.
///
/// The descriptor is a `const RegisteredLanguage` defined in each parser module, so
/// registration never constructs a parser instance. Tree-sitter grammar init only
/// happens when `get_parser()` is called at parse time.
///
/// Usage:
/// ```ignore
/// register_language!(self, builtin::rust, RustParser, DESCRIPTOR);
/// ```
macro_rules! register_language {
    ($registry:expr_2021, $module:path, $parser:ident, $descriptor:ident) => {{
        use $module as _m;
        let desc = &_m::$descriptor;
        $registry.register(desc.extensions, || Ok(Box::new(_m::$parser::new()?)));
        $registry.register_language_id(desc.extensions, desc.language_id);
        $registry.register_descriptor_ref(desc);
    }};
}
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Static snapshot of language metadata, stored inside [`ParserRegistry`].
///
/// This is the authoritative contract for adding a new language to fmm. Each
/// parser module must define `pub(crate) const DESCRIPTOR: RegisteredLanguage`
/// with all fields populated from static data. The `register_language!` macro
/// reads from this const during `ParserRegistry::register_builtin()`, so no
/// parser instance is constructed until parse time.
///
/// Downstream subsystems (config, dependency resolution, glossary, call-site
/// analysis) consume descriptors from the registry instead of hardcoded match
/// arms, so adding a new `DESCRIPTOR` const automatically extends them.
#[derive(Debug)]
pub struct RegisteredLanguage {
    /// Canonical language identifier (e.g. `"rust"`, `"python"`).
    pub language_id: &'static str,
    /// All file extensions handled by this language (without leading dot).
    pub extensions: &'static [&'static str],
    /// Re-export hub filenames (e.g. `["__init__.py"]`, `["mod.rs"]`).
    pub reexport_filenames: &'static [&'static str],
    /// Language-specific test file naming conventions.
    pub test_patterns: LanguageTestPatterns,
}

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
    /// ALP-922: kind tag for nested symbols inside function bodies.
    /// "nested-fn" = depth-1 function declaration inside a function body.
    /// "closure-state" = depth-1 non-trivial var/const/let prologue declaration.
    /// None = regular top-level export or class method (existing behavior).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl ExportEntry {
    pub fn new(name: String, start_line: usize, end_line: usize) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: None,
            kind: None,
        }
    }

    /// Create a method entry belonging to a parent class.
    pub fn method(name: String, start_line: usize, end_line: usize, parent_class: String) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: Some(parent_class),
            kind: None,
        }
    }

    /// Create a depth-1 nested function declaration inside a function body.
    pub fn nested_fn(name: String, start_line: usize, end_line: usize, parent_fn: String) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: Some(parent_fn),
            kind: Some("nested-fn".to_string()),
        }
    }

    /// Create a depth-1 non-trivial var/const/let prologue declaration inside a function body.
    pub fn closure_state(
        name: String,
        start_line: usize,
        end_line: usize,
        parent_fn: String,
    ) -> Self {
        Self {
            name,
            start_line,
            end_line,
            parent_class: Some(parent_fn),
            kind: Some("closure-state".to_string()),
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
    /// Descriptor snapshots collected at registration time.
    descriptors: Vec<RegisteredLanguage>,
    /// Derived O(1) lookup: all known source file extensions.
    source_extensions: HashSet<String>,
    /// Derived O(1) lookup: all known re-export hub filenames.
    reexport_filenames: HashSet<String>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            language_ids: HashMap::new(),
            descriptors: Vec::new(),
            source_extensions: HashSet::new(),
            reexport_filenames: HashSet::new(),
        }
    }

    /// Create a registry pre-loaded with all builtin parsers.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtin();
        registry.build_lookup_tables();
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
    ///
    /// Each language provides a `const DESCRIPTOR: RegisteredLanguage` with static
    /// metadata. No parser instances are constructed during registration; tree-sitter
    /// grammar init only happens lazily when `get_parser()` is called at parse time.
    fn register_builtin(&mut self) {
        // TypeScript / JavaScript (ALP-753: split TS and TSX into separate parsers)
        register_language!(self, builtin::typescript, TypeScriptParser, TS_DESCRIPTOR);

        // TSX / JSX uses LANGUAGE_TSX grammar for correct angle-bracket parsing.
        // Handled separately because the constructor is new_tsx(), not new().
        {
            let desc = &builtin::typescript::TSX_DESCRIPTOR;
            self.register(desc.extensions, || {
                Ok(Box::new(builtin::typescript::TypeScriptParser::new_tsx()?))
            });
            self.register_language_id(desc.extensions, desc.language_id);
            self.register_descriptor_ref(desc);
        }

        register_language!(self, builtin::python, PythonParser, DESCRIPTOR);
        register_language!(self, builtin::rust, RustParser, DESCRIPTOR);
        register_language!(self, builtin::go, GoParser, DESCRIPTOR);
        register_language!(self, builtin::java, JavaParser, DESCRIPTOR);
        register_language!(self, builtin::cpp, CppParser, DESCRIPTOR);
        register_language!(self, builtin::csharp, CSharpParser, DESCRIPTOR);
        register_language!(self, builtin::ruby, RubyParser, DESCRIPTOR);
        register_language!(self, builtin::php, PhpParser, DESCRIPTOR);
        register_language!(self, builtin::c, CParser, DESCRIPTOR);
        register_language!(self, builtin::zig, ZigParser, DESCRIPTOR);
        register_language!(self, builtin::lua, LuaParser, DESCRIPTOR);
        register_language!(self, builtin::scala, ScalaParser, DESCRIPTOR);
        register_language!(self, builtin::swift, SwiftParser, DESCRIPTOR);
        register_language!(self, builtin::kotlin, KotlinParser, DESCRIPTOR);
        register_language!(self, builtin::dart, DartParser, DESCRIPTOR);
        register_language!(self, builtin::elixir, ElixirParser, DESCRIPTOR);
    }

    fn register_language_id(&mut self, extensions: &[&str], language_id: &'static str) {
        for ext in extensions {
            self.language_ids.insert(ext.to_string(), language_id);
        }
    }

    /// Store a [`RegisteredLanguage`] descriptor from a const reference.
    ///
    /// Copies the static data from the const into the descriptor vec. No parser
    /// instance is needed since all descriptor fields are `&'static`.
    fn register_descriptor_ref(&mut self, desc: &RegisteredLanguage) {
        self.descriptors.push(RegisteredLanguage {
            language_id: desc.language_id,
            extensions: desc.extensions,
            reexport_filenames: desc.reexport_filenames,
            test_patterns: LanguageTestPatterns {
                filename_suffixes: desc.test_patterns.filename_suffixes,
                filename_prefixes: desc.test_patterns.filename_prefixes,
                test_symbol_prefixes: desc.test_patterns.test_symbol_prefixes,
            },
        });
    }

    /// Build derived O(1) lookup tables from the collected descriptors.
    ///
    /// Called once by [`with_builtins`] after all parsers are registered.
    fn build_lookup_tables(&mut self) {
        for desc in &self.descriptors {
            for ext in desc.extensions {
                self.source_extensions.insert(ext.to_string());
            }
            for filename in desc.reexport_filenames {
                self.reexport_filenames.insert(filename.to_string());
            }
        }
    }

    /// All registered source file extensions.
    ///
    /// Replaces the hardcoded extension array in `config/mod.rs`
    /// (`default_languages`) and `dependency_matcher.rs` (`strip_source_ext`).
    pub fn source_extensions(&self) -> &HashSet<String> {
        &self.source_extensions
    }

    /// Check if a filename is a re-export hub for any registered language.
    ///
    /// Replaces the hardcoded `matches!` in `mcp/tools/common.rs`.
    pub fn is_reexport_file(&self, filename: &str) -> bool {
        self.reexport_filenames.contains(filename)
    }

    /// Check whether `file_path` is a language-specific test file.
    ///
    /// Only applies filename suffix/prefix patterns that are specific to the
    /// file's extension (e.g. `_test.go` only triggers for `.go` files).
    /// Replaces hardcoded language checks in `glossary_builder.rs`.
    pub fn is_language_test_file(&self, file_path: &str) -> bool {
        let filename = file_path.rsplit('/').next().unwrap_or(file_path);
        for desc in &self.descriptors {
            let ext_matches = desc
                .extensions
                .iter()
                .any(|ext| filename.ends_with(&format!(".{}", ext)));
            if !ext_matches {
                continue;
            }
            for suffix in desc.test_patterns.filename_suffixes {
                if filename.ends_with(suffix) {
                    return true;
                }
            }
            for prefix in desc.test_patterns.filename_prefixes {
                if filename.starts_with(prefix) {
                    return true;
                }
            }
        }
        false
    }

    /// All registered language descriptors.
    pub fn descriptors(&self) -> &[RegisteredLanguage] {
        &self.descriptors
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

    #[test]
    fn registry_source_extensions_covers_all_builtins() {
        let registry = ParserRegistry::with_builtins();
        let exts = registry.source_extensions();
        // Spot-check a selection across all language families
        for ext in [
            "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "cs", "rb", "php", "c", "h",
            "zig", "lua", "scala", "swift", "kt", "dart", "ex",
        ] {
            assert!(
                exts.contains(&ext.to_string()),
                "source_extensions missing: .{ext}"
            );
        }
    }

    #[test]
    fn registry_is_reexport_file_detects_hubs() {
        let registry = ParserRegistry::with_builtins();
        assert!(
            registry.is_reexport_file("__init__.py"),
            "__init__.py should be reexport hub"
        );
        assert!(
            registry.is_reexport_file("mod.rs"),
            "mod.rs should be reexport hub"
        );
        assert!(
            registry.is_reexport_file("index.ts"),
            "index.ts should be reexport hub"
        );
        assert!(
            registry.is_reexport_file("index.js"),
            "index.js should be reexport hub"
        );
        assert!(
            !registry.is_reexport_file("main.rs"),
            "main.rs should NOT be reexport hub"
        );
        assert!(
            !registry.is_reexport_file("lib.py"),
            "lib.py should NOT be reexport hub"
        );
    }

    #[test]
    fn registry_is_language_test_file_detects_patterns() {
        let registry = ParserRegistry::with_builtins();
        assert!(
            registry.is_language_test_file("src/foo_test.go"),
            "_test.go suffix"
        );
        assert!(
            registry.is_language_test_file("src/foo_test.rs"),
            "_test.rs suffix"
        );
        assert!(
            registry.is_language_test_file("src/test_foo.py"),
            "test_ prefix in .py"
        );
        assert!(
            !registry.is_language_test_file("src/main.rs"),
            "main.rs not a test"
        );
        assert!(
            !registry.is_language_test_file("src/server.go"),
            "server.go not a test"
        );
    }

    #[test]
    fn registry_has_correct_descriptor_count() {
        let registry = ParserRegistry::with_builtins();
        // 17 builtin parsers: 16 regular + 1 TSX variant = 18 descriptors
        // (TypeScript = TS + TSX = 2 descriptors)
        assert!(
            registry.descriptors().len() >= 17,
            "expected at least 17 descriptors, got {}",
            registry.descriptors().len()
        );
    }

    /// Guard: every parser factory extension must have a corresponding descriptor.
    ///
    /// If this test fails, a parser was registered via `register()` without a
    /// matching `register_descriptor_ref()` call. Use the `register_language!`
    /// macro to ensure both are always paired.
    #[test]
    fn all_parsers_have_descriptors() {
        let registry = ParserRegistry::with_builtins();
        let descriptor_exts: HashSet<&str> = registry
            .descriptors()
            .iter()
            .flat_map(|d| d.extensions.iter().copied())
            .collect();

        for ext in registry.extensions() {
            assert!(
                descriptor_exts.contains(ext),
                "Extension '{ext}' has a parser factory but no descriptor. \
                 Use register_language!() to register both together."
            );
        }
    }

    /// Guard: descriptor extensions and parser factory extensions must be identical.
    ///
    /// Catches stale descriptors that reference extensions removed from factory
    /// registration, or typos in descriptor extension lists.
    #[test]
    fn descriptor_extensions_match_parser_extensions() {
        let registry = ParserRegistry::with_builtins();
        let factory_exts: HashSet<&str> = registry.extensions().into_iter().collect();
        let descriptor_exts: HashSet<&str> = registry
            .descriptors()
            .iter()
            .flat_map(|d| d.extensions.iter().copied())
            .collect();

        for ext in &descriptor_exts {
            assert!(
                factory_exts.contains(ext),
                "Descriptor declares extension '{ext}' but no parser factory is registered for it."
            );
        }

        for ext in &factory_exts {
            assert!(
                descriptor_exts.contains(ext),
                "Factory registered extension '{ext}' but no descriptor declares it."
            );
        }

        assert_eq!(
            factory_exts, descriptor_exts,
            "Factory extensions and descriptor extensions must be identical sets."
        );
    }

    /// Guard: the hardcoded `default_languages()` in config must stay in sync
    /// with the extensions reported by all registered builtin parsers.
    ///
    /// Cross-module verification; config/mod.rs has the canonical version but
    /// this test provides a safety net at the parser layer.
    #[test]
    fn default_languages_matches_registry() {
        let registry = ParserRegistry::with_builtins();
        let from_registry: std::collections::BTreeSet<String> =
            registry.source_extensions().iter().cloned().collect();
        let config = crate::config::Config::default();

        assert_eq!(
            from_registry, config.languages,
            "ParserRegistry source_extensions() is out of sync with Config::default().languages"
        );
    }
}
