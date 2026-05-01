use crate::parser::{LanguageTestPatterns, ParserRegistry, RegisteredLanguage};
use std::collections::{BTreeMap, HashSet};

/// Static test file and test symbol conventions supplied by a convention plugin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConventionTestPatterns {
    /// Path segments that indicate a test file, for example "/test/".
    pub path_contains: &'static [&'static str],
    /// Filename suffixes that indicate a test file, for example ".spec.ts".
    pub filename_suffixes: &'static [&'static str],
    /// Filename prefixes that indicate a test file, for example "test_".
    pub filename_prefixes: &'static [&'static str],
    /// Symbol name prefixes that indicate a test export, for example "test_".
    pub test_symbol_prefixes: &'static [&'static str],
}

impl ConventionTestPatterns {
    pub const EMPTY: Self = Self {
        path_contains: &[],
        filename_suffixes: &[],
        filename_prefixes: &[],
        test_symbol_prefixes: &[],
    };
}

/// Static project convention contract.
///
/// Convention plugins describe framework or project knowledge that is outside
/// parser construction. Implementations expose static data so registry work can
/// collect conventions without constructing language parsers.
pub trait ConventionPlugin: Send + Sync + 'static {
    fn id() -> &'static str;

    fn languages() -> &'static [&'static str] {
        &[]
    }

    fn enablers() -> &'static [&'static str] {
        &[]
    }

    fn entry_patterns() -> &'static [&'static str] {
        &[]
    }

    fn generated_patterns() -> &'static [&'static str] {
        &[]
    }

    fn virtual_module_prefixes() -> &'static [&'static str] {
        &[]
    }

    fn always_used_symbols() -> &'static [&'static str] {
        &[]
    }

    fn test_patterns() -> ConventionTestPatterns {
        ConventionTestPatterns::EMPTY
    }
}

/// fmm-owned test file conventions that are not language parser conventions.
pub struct FmmTestFileConvention;

impl ConventionPlugin for FmmTestFileConvention {
    fn id() -> &'static str {
        "fmm-test-files"
    }

    fn test_patterns() -> ConventionTestPatterns {
        ConventionTestPatterns {
            path_contains: &["/tests/", "/test/", "/__tests__/"],
            filename_suffixes: &[],
            filename_prefixes: &[],
            test_symbol_prefixes: &[],
        }
    }
}

/// Static convention metadata captured during registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredConvention {
    pub id: &'static str,
    pub languages: &'static [&'static str],
    pub enablers: &'static [&'static str],
    pub entry_patterns: &'static [&'static str],
    pub generated_patterns: &'static [&'static str],
    pub virtual_module_prefixes: &'static [&'static str],
    pub always_used_symbols: &'static [&'static str],
    pub test_patterns: ConventionTestPatterns,
}

impl RegisteredConvention {
    fn from_plugin<T: ConventionPlugin>() -> Self {
        Self {
            id: T::id(),
            languages: T::languages(),
            enablers: T::enablers(),
            entry_patterns: T::entry_patterns(),
            generated_patterns: T::generated_patterns(),
            virtual_module_prefixes: T::virtual_module_prefixes(),
            always_used_symbols: T::always_used_symbols(),
            test_patterns: T::test_patterns(),
        }
    }
}

/// Adapter that exposes plugin conventions and parser descriptor conventions through one path.
pub struct ConventionRegistry<'a> {
    parser_registry: &'a ParserRegistry,
    plugins: BTreeMap<&'static str, RegisteredConvention>,
}

impl<'a> ConventionRegistry<'a> {
    pub fn new(parser_registry: &'a ParserRegistry) -> Self {
        Self {
            parser_registry,
            plugins: BTreeMap::new(),
        }
    }

    pub fn with_builtin_conventions(parser_registry: &'a ParserRegistry) -> Self {
        let mut registry = Self::new(parser_registry);
        registry.register::<FmmTestFileConvention>();
        registry
    }

    /// Register a convention plugin by type.
    ///
    /// If the id already exists, the new static descriptor replaces the old one
    /// and the previous descriptor is returned.
    pub fn register<T: ConventionPlugin>(&mut self) -> Option<RegisteredConvention> {
        let plugin = RegisteredConvention::from_plugin::<T>();
        self.plugins.insert(plugin.id, plugin)
    }

    pub fn plugin(&self, id: &str) -> Option<&RegisteredConvention> {
        self.plugins.get(id)
    }

    pub fn plugins(&self) -> impl Iterator<Item = &RegisteredConvention> + '_ {
        self.plugins.values()
    }

    pub fn parser_registry(&self) -> &ParserRegistry {
        self.parser_registry
    }

    pub fn source_extensions(&self) -> &HashSet<String> {
        self.parser_registry.source_extensions()
    }

    pub fn is_reexport_file(&self, filename: &str) -> bool {
        self.parser_registry.is_reexport_file(filename)
    }

    pub fn is_language_test_file(&self, file_path: &str) -> bool {
        self.parser_registry.is_language_test_file(file_path)
    }

    /// ALP-2123: parser descriptors remain the source for language test
    /// filename patterns while fmm-owned project conventions live as plugins.
    pub fn is_test_file(&self, file_path: &str) -> bool {
        if self.is_language_test_file(file_path) {
            return true;
        }
        self.plugins
            .values()
            .any(|plugin| test_patterns_match_file(file_path, &plugin.test_patterns))
    }

    pub fn language_descriptors(&self) -> &[RegisteredLanguage] {
        self.parser_registry.descriptors()
    }

    pub fn language_test_patterns(&self) -> impl Iterator<Item = &LanguageTestPatterns> + '_ {
        self.parser_registry
            .descriptors()
            .iter()
            .map(|desc| &desc.test_patterns)
    }
}

fn test_patterns_match_file(file_path: &str, patterns: &ConventionTestPatterns) -> bool {
    patterns
        .path_contains
        .iter()
        .any(|pattern| path_pattern_matches(file_path, pattern))
        || filename_patterns_match(file_path, patterns)
}

fn path_pattern_matches(file_path: &str, pattern: &str) -> bool {
    if let Some(root_pattern) = pattern.strip_prefix('/') {
        file_path.starts_with(root_pattern) || file_path.contains(pattern)
    } else {
        file_path.contains(pattern)
    }
}

fn filename_patterns_match(file_path: &str, patterns: &ConventionTestPatterns) -> bool {
    let filename = file_path.rsplit('/').next().unwrap_or(file_path);
    patterns
        .filename_suffixes
        .iter()
        .any(|suffix| filename.ends_with(suffix))
        || patterns
            .filename_prefixes
            .iter()
            .any(|prefix| filename.starts_with(prefix))
}
