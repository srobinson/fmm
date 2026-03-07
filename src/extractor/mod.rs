use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::parser::{Metadata, ParseResult, ParserRegistry};

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
