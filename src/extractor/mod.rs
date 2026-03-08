use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::parser::{Metadata, ParseResult, Parser, ParserRegistry};

pub struct FileProcessor {
    registry: ParserRegistry,
}

impl FileProcessor {
    pub fn new(_root: &Path) -> Self {
        Self {
            registry: ParserRegistry::with_builtins(),
        }
    }

    /// Extract metadata from a file (public for manifest/search).
    pub fn extract_metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let content = fs::read_to_string(path)?;
        let result = self.parse_content(path, &content)?;
        Ok(Some(result.metadata))
    }

    /// Parse a source file and return the full ParseResult (metadata + custom_fields).
    ///
    /// Preserves custom_fields (e.g. TypeScript function_names, decorators) needed
    /// by the SQLite write path.
    pub fn parse(&self, path: &Path) -> Result<ParseResult> {
        let content = fs::read_to_string(path)?;
        self.parse_content(path, &content)
    }

    /// Single-pass parse: metadata + custom fields from one tree-sitter invocation.
    fn parse_content(&self, path: &Path, content: &str) -> Result<ParseResult> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let mut parser = self.registry.get_parser(extension)?;
        parser.parse_file(content, path)
    }
}

/// Per-thread parser cache for the parallel parse phase.
///
/// Constructed once per rayon worker thread via `map_init`. Holds one parser
/// instance per extension so tree-sitter setup and query compilation happen
/// at most once per thread instead of once per file.
pub struct ParserCache {
    registry: ParserRegistry,
    parsers: HashMap<String, Box<dyn Parser>>,
}

impl Default for ParserCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserCache {
    pub fn new() -> Self {
        Self {
            registry: ParserRegistry::with_builtins(),
            parsers: HashMap::new(),
        }
    }

    /// Parse `path`, reusing the cached parser for its extension.
    pub fn parse_file(&mut self, path: &Path) -> Result<ParseResult> {
        let content = fs::read_to_string(path)?;
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?
            .to_string();

        if !self.parsers.contains_key(&extension) {
            let parser = self.registry.get_parser(&extension)?;
            self.parsers.insert(extension.clone(), parser);
        }

        self.parsers
            .get_mut(&extension)
            .unwrap()
            .parse_file(&content, path)
    }
}
