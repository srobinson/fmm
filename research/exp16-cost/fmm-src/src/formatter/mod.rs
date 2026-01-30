use chrono::Utc;
use std::collections::HashMap;

use crate::config::Language;
use crate::parser::Metadata;

pub struct Frontmatter {
    file_path: String,
    metadata: Metadata,
    language: Language,
    modified: String,
    version: Option<String>,
    /// Language-specific custom fields, keyed by language ID (e.g., "rust", "python")
    custom_fields: Option<(String, HashMap<String, serde_json::Value>)>,
}

impl Frontmatter {
    pub fn new(file_path: String, metadata: Metadata, language: Language) -> Self {
        Self {
            file_path,
            metadata,
            language,
            modified: Utc::now().format("%Y-%m-%d").to_string(),
            version: None,
            custom_fields: None,
        }
    }

    /// Set the format version (e.g., "v0.2").
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    /// Set language-specific custom fields to render as a nested section.
    pub fn with_custom_fields(
        mut self,
        language_id: Option<&str>,
        fields: Option<&HashMap<String, serde_json::Value>>,
    ) -> Self {
        if let (Some(lang), Some(f)) = (language_id, fields) {
            if !f.is_empty() {
                self.custom_fields = Some((lang.to_string(), f.clone()));
            }
        }
        self
    }

    pub fn render(&self) -> String {
        let prefix = self.language.comment_prefix();

        let mut lines = vec![format!("{} --- FMM ---", prefix)];

        // Version (if set)
        if let Some(ref version) = self.version {
            lines.push(format!("{} fmm: {}", prefix, version));
        }

        // File path
        lines.push(format!("{} file: {}", prefix, self.file_path));

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

        // Language-specific section
        if let Some((ref lang_id, ref fields)) = self.custom_fields {
            lines.push(format!("{} {}:", prefix, lang_id));
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            for key in keys {
                let value = &fields[key];
                lines.push(format!("{}   {}: {}", prefix, key, format_value(value)));
            }
        }

        // Closing
        lines.push(format!("{} ---", prefix));

        lines.join("\n")
    }
}

/// Format a serde_json::Value for YAML-like frontmatter output.
fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Object(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
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

        assert!(rendered.contains("// --- FMM ---"));
        assert!(rendered.contains("// file: src/auth/session.ts"));
        assert!(rendered.contains("// exports: [createSession, validateSession]"));
        assert!(rendered.contains("// imports: [jwt, redis]"));
        assert!(rendered.contains("// dependencies: [./types, ./config]"));
        assert!(rendered.contains("// loc: 234"));
        assert!(rendered.contains("// modified:"));
        // No version by default
        assert!(!rendered.contains("// fmm:"));
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

        assert!(rendered.contains("# --- FMM ---"));
        assert!(rendered.contains("# file: src/processor.py"));
        assert!(rendered.contains("# exports: [process_data]"));
        assert!(rendered.contains("# imports: [pandas]"));
    }

    #[test]
    fn test_frontmatter_with_version() {
        let metadata = Metadata {
            exports: vec!["foo".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata, Language::TypeScript)
            .with_version("v0.2");

        let rendered = fm.render();
        assert!(rendered.contains("// fmm: v0.2"));
        // Version should come right after the header
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "// --- FMM ---");
        assert_eq!(lines[1], "// fmm: v0.2");
    }

    #[test]
    fn test_frontmatter_without_version() {
        let metadata = Metadata {
            exports: vec!["foo".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata, Language::TypeScript);

        let rendered = fm.render();
        assert!(!rendered.contains("fmm:"));
    }

    #[test]
    fn test_frontmatter_with_rust_custom_fields() {
        let metadata = Metadata {
            exports: vec!["MyStruct".to_string()],
            imports: vec!["std".to_string()],
            dependencies: vec![],
            loc: 50,
        };

        let mut custom = HashMap::new();
        custom.insert(
            "unsafe_blocks".to_string(),
            serde_json::Value::Number(3.into()),
        );
        custom.insert(
            "derives".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("Debug".to_string()),
                serde_json::Value::String("Clone".to_string()),
            ]),
        );

        let fm = Frontmatter::new("src/lib.rs".to_string(), metadata, Language::Rust)
            .with_version("v0.2")
            .with_custom_fields(Some("rust"), Some(&custom));

        let rendered = fm.render();
        assert!(rendered.contains("// rust:"));
        assert!(rendered.contains("//   derives: [Debug, Clone]"));
        assert!(rendered.contains("//   unsafe_blocks: 3"));
    }

    #[test]
    fn test_frontmatter_with_python_custom_fields() {
        let metadata = Metadata {
            exports: vec!["MyClass".to_string()],
            imports: vec!["flask".to_string()],
            dependencies: vec![],
            loc: 100,
        };

        let mut custom = HashMap::new();
        custom.insert(
            "decorators".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("staticmethod".to_string()),
                serde_json::Value::String("property".to_string()),
            ]),
        );

        let fm = Frontmatter::new("app.py".to_string(), metadata, Language::Python)
            .with_version("v0.2")
            .with_custom_fields(Some("python"), Some(&custom));

        let rendered = fm.render();
        assert!(rendered.contains("# python:"));
        assert!(rendered.contains("#   decorators: [staticmethod, property]"));
    }

    #[test]
    fn test_frontmatter_no_custom_fields_when_none() {
        let metadata = Metadata {
            exports: vec!["foo".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata, Language::TypeScript)
            .with_custom_fields(None, None);

        let rendered = fm.render();
        assert!(!rendered.contains("typescript:"));
    }

    #[test]
    fn test_frontmatter_no_custom_fields_when_empty() {
        let metadata = Metadata {
            exports: vec!["foo".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let empty: HashMap<String, serde_json::Value> = HashMap::new();
        let fm = Frontmatter::new("test.ts".to_string(), metadata, Language::TypeScript)
            .with_custom_fields(Some("typescript"), Some(&empty));

        let rendered = fm.render();
        assert!(!rendered.contains("typescript:"));
    }

    #[test]
    fn test_format_value_primitives() {
        assert_eq!(
            format_value(&serde_json::Value::String("hello".to_string())),
            "hello"
        );
        assert_eq!(format_value(&serde_json::Value::Number(42.into())), "42");
        assert_eq!(format_value(&serde_json::Value::Bool(true)), "true");
        assert_eq!(format_value(&serde_json::Value::Null), "null");
    }

    #[test]
    fn test_format_value_array() {
        let arr = serde_json::Value::Array(vec![
            serde_json::Value::String("a".to_string()),
            serde_json::Value::String("b".to_string()),
        ]);
        assert_eq!(format_value(&arr), "[a, b]");
    }
}
