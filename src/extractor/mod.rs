use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::formatter::Frontmatter;
use crate::parser::{Metadata, ParseResult, ParserRegistry};

/// Strip the `modified:` line so date-only changes don't trigger rewrites.
fn content_without_modified(s: &str) -> String {
    s.lines()
        .filter(|line| !line.starts_with("modified:"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct FileProcessor {
    root: std::path::PathBuf,
    registry: ParserRegistry,
}

/// Returns the sidecar path for a source file: `foo.rs` → `foo.rs.fmm`
pub fn sidecar_path_for(path: &Path) -> PathBuf {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(".fmm");
    PathBuf::from(sidecar)
}

impl FileProcessor {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            registry: ParserRegistry::with_builtins(),
        }
    }

    pub fn generate(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let sidecar = sidecar_path_for(path);
        if sidecar.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let result = self.parse_content(path, &content)?;
        let yaml = self.format_sidecar(path, &result.metadata, result.custom_fields.as_ref())?;

        if dry_run {
            return Ok(Some(format!("Would write: {}", sidecar.display())));
        }

        fs::write(&sidecar, &yaml)?;
        Ok(Some("Wrote sidecar".to_string()))
    }

    pub fn update(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;
        let result = self.parse_content(path, &content)?;
        let new_yaml =
            self.format_sidecar(path, &result.metadata, result.custom_fields.as_ref())?;

        let sidecar = sidecar_path_for(path);
        if sidecar.exists() {
            let old = fs::read_to_string(&sidecar)?;
            if content_without_modified(&old) == content_without_modified(&new_yaml) {
                return Ok(None);
            }
        }

        if dry_run {
            return Ok(Some(format!("Would update: {}", sidecar.display())));
        }

        fs::write(&sidecar, &new_yaml)?;
        Ok(Some("Updated sidecar".to_string()))
    }

    pub fn validate(&self, path: &Path) -> Result<bool> {
        let sidecar = sidecar_path_for(path);
        if !sidecar.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(path)?;
        let result = self.parse_content(path, &content)?;
        let expected =
            self.format_sidecar(path, &result.metadata, result.custom_fields.as_ref())?;
        let actual = fs::read_to_string(&sidecar)?;

        Ok(content_without_modified(actual.trim()) == content_without_modified(expected.trim()))
    }

    /// Delete the sidecar file for a source file.
    pub fn clean(&self, path: &Path) -> Result<bool> {
        let sidecar = sidecar_path_for(path);
        if sidecar.exists() {
            fs::remove_file(&sidecar)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Extract metadata from a file (public for manifest/search)
    pub fn extract_metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let content = fs::read_to_string(path)?;
        let result = self.parse_content(path, &content)?;
        Ok(Some(result.metadata))
    }

    /// Single-pass parse: metadata + custom fields from one tree-sitter invocation.
    fn parse_content(&self, path: &Path, content: &str) -> Result<ParseResult> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let mut parser = self.registry.get_parser(extension)?;
        parser.parse(content)
    }

    fn format_sidecar(
        &self,
        path: &Path,
        metadata: &Metadata,
        custom_fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let relative_path = match path.strip_prefix(&self.root) {
            Ok(rel) => rel,
            Err(_) => path,
        };

        let language_id = self.registry.language_id_for(extension);

        let frontmatter = Frontmatter::new(relative_path.display().to_string(), metadata.clone())
            .with_version("v0.3")
            .with_custom_fields(language_id, custom_fields);

        Ok(format!("{}\n", frontmatter.render()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_path_for() {
        let path = Path::new("src/cli/mod.rs");
        assert_eq!(sidecar_path_for(path), PathBuf::from("src/cli/mod.rs.fmm"));
    }

    #[test]
    fn test_sidecar_path_for_nested() {
        let path = Path::new("/abs/path/to/file.ts");
        assert_eq!(
            sidecar_path_for(path),
            PathBuf::from("/abs/path/to/file.ts.fmm")
        );
    }

    #[test]
    fn content_without_modified_strips_date_line() {
        let with_date = "file: src/foo.rs\nexports:\n  bar: [1, 5]\nloc: 10\nmodified: 2026-01-01";
        let with_different_date =
            "file: src/foo.rs\nexports:\n  bar: [1, 5]\nloc: 10\nmodified: 2026-02-14";
        assert_eq!(
            content_without_modified(with_date),
            content_without_modified(with_different_date)
        );
    }

    #[test]
    fn content_without_modified_detects_real_changes() {
        let v1 = "file: src/foo.rs\nexports:\n  bar: [1, 5]\nloc: 10\nmodified: 2026-01-01";
        let v2 = "file: src/foo.rs\nexports:\n  bar: [1, 5]\n  baz: [6, 10]\nloc: 20\nmodified: 2026-02-14";
        assert_ne!(
            content_without_modified(v1),
            content_without_modified(v2)
        );
    }
}
