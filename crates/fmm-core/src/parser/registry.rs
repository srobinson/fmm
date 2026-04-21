use super::{LanguageTestPatterns, Parser, RegisteredLanguage, builtin};
use anyhow::Result;
use std::collections::{HashMap, HashSet};

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
/// register_language!(self, super::builtin::rust, RustParser, DESCRIPTOR);
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
        // Share the factory via Arc for multiple extensions.
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
        register_language!(
            self,
            super::builtin::typescript,
            TypeScriptParser,
            TS_DESCRIPTOR
        );

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

        register_language!(self, super::builtin::python, PythonParser, DESCRIPTOR);
        register_language!(self, super::builtin::rust, RustParser, DESCRIPTOR);
        register_language!(self, super::builtin::go, GoParser, DESCRIPTOR);
        register_language!(self, super::builtin::java, JavaParser, DESCRIPTOR);
        register_language!(self, super::builtin::cpp, CppParser, DESCRIPTOR);
        register_language!(self, super::builtin::csharp, CSharpParser, DESCRIPTOR);
        register_language!(self, super::builtin::ruby, RubyParser, DESCRIPTOR);
        register_language!(self, super::builtin::php, PhpParser, DESCRIPTOR);
        register_language!(self, super::builtin::c, CParser, DESCRIPTOR);
        register_language!(self, super::builtin::zig, ZigParser, DESCRIPTOR);
        register_language!(self, super::builtin::lua, LuaParser, DESCRIPTOR);
        register_language!(self, super::builtin::scala, ScalaParser, DESCRIPTOR);
        register_language!(self, super::builtin::swift, SwiftParser, DESCRIPTOR);
        register_language!(self, super::builtin::kotlin, KotlinParser, DESCRIPTOR);
        register_language!(self, super::builtin::dart, DartParser, DESCRIPTOR);
        register_language!(self, super::builtin::elixir, ElixirParser, DESCRIPTOR);
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
