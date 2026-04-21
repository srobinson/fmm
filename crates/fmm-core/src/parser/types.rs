use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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

/// Static snapshot of language metadata, stored inside [`ParserRegistry`](crate::parser::ParserRegistry).
///
/// This is the authoritative contract for adding a new language to fmm. Each
/// parser module must define `pub(crate) const DESCRIPTOR: RegisteredLanguage`
/// with all fields populated from static data. The registry reads from this
/// const during builtin registration, so no parser instance is constructed until
/// parse time.
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
    /// Named imports per source module (TS/JS, Python, Rust).
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
