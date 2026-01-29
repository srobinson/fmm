use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::formatter::Frontmatter;
use crate::parser::{Metadata, ParserRegistry};

pub struct FileProcessor {
    config: Config,
    root: std::path::PathBuf,
    registry: ParserRegistry,
}

impl FileProcessor {
    pub fn new(config: &Config, root: &Path) -> Self {
        Self {
            config: config.clone(),
            root: root.to_path_buf(),
            registry: ParserRegistry::with_builtins(),
        }
    }

    pub fn generate(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;

        if has_frontmatter(&content) {
            return Ok(None);
        }

        let code = &content;
        let metadata = self.extract_metadata_from_content(path, code)?;
        let custom_fields = self.extract_custom_fields(path, code);
        let frontmatter = self.format_frontmatter(path, &metadata, custom_fields.as_ref())?;

        if dry_run {
            return Ok(Some(format!("Would add:\n{}", frontmatter)));
        }

        let new_content = format!("{}\n\n{}", frontmatter, content);
        fs::write(path, new_content)?;

        Ok(Some("Added frontmatter".to_string()))
    }

    pub fn update(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;

        let code = if let Some((_, rest)) = extract_frontmatter(&content) {
            rest.clone()
        } else {
            content.clone()
        };

        let metadata = self.extract_metadata_from_content(path, &code)?;
        let custom_fields = self.extract_custom_fields(path, &code);
        let new_frontmatter = self.format_frontmatter(path, &metadata, custom_fields.as_ref())?;

        if let Some((old_fm, rest)) = extract_frontmatter(&content) {
            if old_fm.trim() == new_frontmatter.trim() {
                return Ok(None);
            }

            if dry_run {
                return Ok(Some(format!(
                    "Would update:\n- Old: {}\n+ New: {}",
                    old_fm.lines().count(),
                    new_frontmatter.lines().count()
                )));
            }

            let new_content = format!("{}\n\n{}", new_frontmatter, rest);
            fs::write(path, new_content)?;

            Ok(Some("Updated frontmatter".to_string()))
        } else {
            self.generate(path, dry_run)
        }
    }

    pub fn validate(&self, path: &Path) -> Result<bool> {
        let content = fs::read_to_string(path)?;

        if let Some((old_fm, rest)) = extract_frontmatter(&content) {
            let metadata = self.extract_metadata_from_content(path, &rest)?;
            let custom_fields = self.extract_custom_fields(path, &rest);
            let expected_fm = self.format_frontmatter(path, &metadata, custom_fields.as_ref())?;

            Ok(old_fm.trim() == expected_fm.trim())
        } else {
            Ok(false)
        }
    }

    /// Extract metadata from a file (public for manifest generation)
    pub fn extract_metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let content = std::fs::read_to_string(path)?;
        let code = if let Some((_, rest)) = extract_frontmatter(&content) {
            rest
        } else {
            content
        };
        Ok(Some(self.extract_metadata_from_content(path, &code)?))
    }

    /// Extract custom fields from a file's source code
    pub fn extract_custom_fields(
        &self,
        path: &Path,
        content: &str,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let extension = path.extension().and_then(|ext| ext.to_str())?;
        let mut parser = self.registry.get_parser(extension).ok()?;
        // We need to parse first to populate internal state, then get custom fields
        let _ = parser.parse(content).ok()?;
        parser.custom_fields(content)
    }

    /// Get the language ID for a file extension
    #[allow(dead_code)]
    pub fn language_id_for(&self, path: &Path) -> Option<String> {
        let extension = path.extension().and_then(|ext| ext.to_str())?;
        let parser = self.registry.get_parser(extension).ok()?;
        Some(parser.language_id().to_string())
    }

    fn extract_metadata_from_content(&self, path: &Path, content: &str) -> Result<Metadata> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let mut parser = self.registry.get_parser(extension)?;
        parser.parse(content)
    }

    fn format_frontmatter(
        &self,
        path: &Path,
        metadata: &Metadata,
        custom_fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let language = self
            .config
            .language_from_extension(extension)
            .context("Unsupported language")?;

        let relative_path = match path.strip_prefix(&self.root) {
            Ok(rel) => rel,
            Err(_) => {
                log::warn!(
                    "Failed to strip prefix {:?} from {:?}, using absolute path",
                    self.root,
                    path
                );
                path
            }
        };

        let language_id = self
            .registry
            .get_parser(extension)
            .ok()
            .map(|p| p.language_id().to_string());

        let frontmatter = Frontmatter::new(
            relative_path.display().to_string(),
            metadata.clone(),
            language,
        )
        .with_version("v0.2")
        .with_custom_fields(language_id.as_deref(), custom_fields);

        Ok(frontmatter.render())
    }
}

fn has_frontmatter(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return false;
    }

    let first = lines[0].trim();
    (first.starts_with("//") || first.starts_with("#")) && first.contains("--- FMM ---")
}

fn extract_frontmatter(content: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let first = lines[0].trim();
    if !((first.starts_with("//") || first.starts_with("#")) && first.contains("--- FMM ---")) {
        return None;
    }

    let mut end_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim();
        if (trimmed.starts_with("//") || trimmed.starts_with("#"))
            && trimmed.ends_with("---")
            && !trimmed.contains("FMM")
        {
            end_idx = Some(i);
            break;
        }
    }

    if let Some(end) = end_idx {
        let frontmatter = lines[0..=end].join("\n");
        let rest = if end + 1 < lines.len() {
            lines[end + 1..].join("\n").trim_start().to_string()
        } else {
            String::new()
        };
        Some((frontmatter, rest))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_frontmatter_new_format() {
        let content = r#"// --- FMM ---
// file: test.ts
// exports: [foo]
// ---

export function foo() {}"#;
        assert!(has_frontmatter(content));
    }

    #[test]
    fn test_has_frontmatter_legacy_format_rejected() {
        let content = r#"// ---
// file: test.ts
// exports: [foo]
// ---

export function foo() {}"#;
        assert!(!has_frontmatter(content));
    }

    #[test]
    fn test_has_frontmatter_python() {
        let content = r#"# --- FMM ---
# file: test.py
# exports: [foo]
# ---

def foo(): pass"#;
        assert!(has_frontmatter(content));
    }

    #[test]
    fn test_has_frontmatter_none() {
        let content = "export function foo() {}";
        assert!(!has_frontmatter(content));
    }

    #[test]
    fn test_extract_frontmatter_new_format() {
        let content = r#"// --- FMM ---
// file: test.ts
// exports: [foo]
// ---

export function foo() {}"#;

        let (fm, rest) = extract_frontmatter(content).unwrap();
        assert!(fm.contains("// --- FMM ---"));
        assert!(fm.contains("// exports: [foo]"));
        assert!(rest.contains("export function foo()"));
    }

    #[test]
    fn test_extract_frontmatter_legacy_format_rejected() {
        let content = r#"// ---
// file: test.ts
// exports: [bar]
// ---

export function bar() {}"#;

        assert!(extract_frontmatter(content).is_none());
    }

    #[test]
    fn test_extract_frontmatter_none() {
        let content = "export function foo() {}";
        assert!(extract_frontmatter(content).is_none());
    }
}
