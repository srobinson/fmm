use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::{Config, Language};
use crate::formatter::Frontmatter;
use crate::parser::{Metadata, Parser, TypeScriptParser};

pub struct FileProcessor {
    config: Config,
}

impl FileProcessor {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn generate(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;

        // Check if frontmatter already exists
        if has_frontmatter(&content) {
            return Ok(None); // Skip files that already have frontmatter
        }

        let metadata = self.extract_metadata_from_content(path, &content)?;
        let frontmatter = self.format_frontmatter(path, &metadata)?;

        if dry_run {
            return Ok(Some(format!("Would add:\n{}", frontmatter)));
        }

        // Prepend frontmatter to file
        let new_content = format!("{}\n\n{}", frontmatter, content);
        fs::write(path, new_content)?;

        Ok(Some("Added frontmatter".to_string()))
    }

    pub fn update(&self, path: &Path, dry_run: bool) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;

        // Strip frontmatter to get clean content for metadata extraction
        let code = if let Some((_, rest)) = extract_frontmatter(&content) {
            rest.clone()
        } else {
            content.clone()
        };

        let metadata = self.extract_metadata_from_content(path, &code)?;
        let new_frontmatter = self.format_frontmatter(path, &metadata)?;

        if let Some((old_fm, rest)) = extract_frontmatter(&content) {
            // Compare old and new
            if old_fm.trim() == new_frontmatter.trim() {
                return Ok(None); // No changes needed
            }

            if dry_run {
                return Ok(Some(format!(
                    "Would update:\n- Old: {}\n+ New: {}",
                    old_fm.lines().count(),
                    new_frontmatter.lines().count()
                )));
            }

            // Replace frontmatter
            let new_content = format!("{}\n\n{}", new_frontmatter, rest);
            fs::write(path, new_content)?;

            Ok(Some("Updated frontmatter".to_string()))
        } else {
            // No existing frontmatter, add it
            self.generate(path, dry_run)
        }
    }

    pub fn validate(&self, path: &Path) -> Result<bool> {
        let content = fs::read_to_string(path)?;

        if let Some((old_fm, rest)) = extract_frontmatter(&content) {
            let metadata = self.extract_metadata_from_content(path, &rest)?;
            let expected_fm = self.format_frontmatter(path, &metadata)?;

            Ok(old_fm.trim() == expected_fm.trim())
        } else {
            // No frontmatter exists
            Ok(false)
        }
    }

    /// Extract metadata from a file (public for manifest generation)
    pub fn extract_metadata(&self, path: &Path) -> Result<Option<Metadata>> {
        let content = std::fs::read_to_string(path)?;
        // Strip existing frontmatter if present to get accurate metadata
        let code = if let Some((_, rest)) = extract_frontmatter(&content) {
            rest
        } else {
            content
        };
        Ok(Some(self.extract_metadata_from_content(path, &code)?))
    }

    fn extract_metadata_from_content(&self, path: &Path, content: &str) -> Result<Metadata> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let language = self
            .config
            .language_from_extension(extension)
            .context("Unsupported language")?;

        match language {
            Language::TypeScript | Language::JavaScript => {
                let mut parser = TypeScriptParser::new()?;
                parser.parse(content)
            }
            _ => anyhow::bail!("Parser not yet implemented for {:?}", language),
        }
    }

    fn format_frontmatter(&self, path: &Path, metadata: &Metadata) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .context("Invalid file extension")?;

        let language = self
            .config
            .language_from_extension(extension)
            .context("Unsupported language")?;

        let relative_path = path
            .strip_prefix(std::env::current_dir()?)
            .unwrap_or(path);

        let frontmatter = Frontmatter::new(
            relative_path.display().to_string(),
            metadata.clone(),
            language,
        );

        Ok(frontmatter.render())
    }
}

fn has_frontmatter(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return false;
    }

    // Check if first line is a comment with "--- FMM ---"
    let first = lines[0].trim();
    (first.starts_with("//") || first.starts_with("#")) && first.contains("--- FMM ---")
}

fn extract_frontmatter(content: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // Only extract if starts with FMM header
    let first = lines[0].trim();
    if !((first.starts_with("//") || first.starts_with("#")) && first.contains("--- FMM ---")) {
        return None;
    }

    // Find the closing "---" marker
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
        // Old format without "FMM" is NOT supported
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
        // Old format without "FMM" is NOT supported
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
