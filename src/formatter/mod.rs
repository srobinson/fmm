use chrono::Utc;

use crate::config::Language;
use crate::parser::Metadata;

pub struct Frontmatter {
    file_path: String,
    metadata: Metadata,
    language: Language,
    modified: String,
}

impl Frontmatter {
    pub fn new(file_path: String, metadata: Metadata, language: Language) -> Self {
        Self {
            file_path,
            metadata,
            language,
            modified: Utc::now().format("%Y-%m-%d").to_string(),
        }
    }

    pub fn render(&self) -> String {
        let prefix = self.language.comment_prefix();

        let mut lines = vec![
            format!("{} ---", prefix),
            format!("{} file: {}", prefix, self.file_path),
        ];

        // Exports
        if !self.metadata.exports.is_empty() {
            lines.push(format!(
                "{} exports: [{}]",
                prefix,
                self.metadata.exports.join(", ")
            ));
        }

        // Imports (external packages only)
        if !self.metadata.imports.is_empty() {
            lines.push(format!(
                "{} imports: [{}]",
                prefix,
                self.metadata.imports.join(", ")
            ));
        }

        // Dependencies (local relative imports)
        if !self.metadata.dependencies.is_empty() {
            lines.push(format!(
                "{} dependencies: [{}]",
                prefix,
                self.metadata.dependencies.join(", ")
            ));
        }

        // LOC
        lines.push(format!("{} loc: {}", prefix, self.metadata.loc));

        // Modified date
        lines.push(format!("{} modified: {}", prefix, self.modified));

        // Closing
        lines.push(format!("{} ---", prefix));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_frontmatter() {
        let metadata = Metadata {
            exports: vec!["createSession".to_string(), "validateSession".to_string()],
            imports: vec!["jwt".to_string(), "redis".to_string()],
            dependencies: vec!["./types".to_string(), "./config".to_string()],
            loc: 234,
        };

        let fm = Frontmatter::new(
            "src/auth/session.ts".to_string(),
            metadata,
            Language::TypeScript,
        );

        let rendered = fm.render();

        assert!(rendered.contains("// ---"));
        assert!(rendered.contains("// file: src/auth/session.ts"));
        assert!(rendered.contains("// exports: [createSession, validateSession]"));
        assert!(rendered.contains("// imports: [jwt, redis]"));
        assert!(rendered.contains("// dependencies: [./types, ./config]"));
        assert!(rendered.contains("// loc: 234"));
        assert!(rendered.contains("// modified:"));
    }

    #[test]
    fn test_python_frontmatter() {
        let metadata = Metadata {
            exports: vec!["process_data".to_string()],
            imports: vec!["pandas".to_string()],
            dependencies: vec!["./utils".to_string()],
            loc: 156,
        };

        let fm = Frontmatter::new("src/processor.py".to_string(), metadata, Language::Python);

        let rendered = fm.render();

        assert!(rendered.contains("# ---"));
        assert!(rendered.contains("# file: src/processor.py"));
        assert!(rendered.contains("# exports: [process_data]"));
        assert!(rendered.contains("# imports: [pandas]"));
    }
}
