use chrono::Utc;
use std::collections::HashMap;

use crate::parser::Metadata;

pub struct Frontmatter {
    file_path: String,
    metadata: Metadata,
    modified: String,
    version: Option<String>,
    /// Language-specific custom fields, keyed by language ID (e.g., "rust", "python")
    custom_fields: Option<(String, HashMap<String, serde_json::Value>)>,
}

impl Frontmatter {
    pub fn new(file_path: String, metadata: Metadata) -> Self {
        Self {
            file_path,
            metadata,
            modified: Utc::now().format("%Y-%m-%d").to_string(),
            version: None,
            custom_fields: None,
        }
    }

    /// Set the format version (e.g., "v0.3").
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
        let mut lines = Vec::new();

        // YAML document start
        lines.push("---".to_string());

        // File path (first — most useful for LLM orientation)
        lines.push(format!("file: {}", yaml_escape(&self.file_path)));

        // Version
        if let Some(ref version) = self.version {
            lines.push(format!("fmm: {}", version));
        }

        // Exports as YAML map with [start, end] line ranges
        if !self.metadata.exports.is_empty() {
            lines.push("exports:".to_string());
            for entry in &self.metadata.exports {
                lines.push(format!(
                    "  {}: [{}, {}]",
                    yaml_escape(&entry.name),
                    entry.start_line,
                    entry.end_line
                ));
            }
        }

        // Imports (external packages only)
        if !self.metadata.imports.is_empty() {
            let items: Vec<_> = self
                .metadata
                .imports
                .iter()
                .map(|s| yaml_escape(s))
                .collect();
            lines.push(format!("imports: [{}]", items.join(", ")));
        }

        // Dependencies (local relative imports)
        if !self.metadata.dependencies.is_empty() {
            let items: Vec<_> = self
                .metadata
                .dependencies
                .iter()
                .map(|s| yaml_escape(s))
                .collect();
            lines.push(format!("dependencies: [{}]", items.join(", ")));
        }

        // LOC
        lines.push(format!("loc: {}", self.metadata.loc));

        // Modified date
        lines.push(format!("modified: {}", self.modified));

        // Language-specific section
        if let Some((ref lang_id, ref fields)) = self.custom_fields {
            lines.push(format!("{}:", lang_id));
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            for key in keys {
                let value = &fields[key];
                lines.push(format!("  {}: {}", key, format_value(value)));
            }
        }

        lines.join("\n")
    }
}

/// Quote a string if it contains YAML-special characters that would break parsing.
/// Returns the original string unmodified when safe, or wraps it in single quotes.
fn yaml_escape(s: &str) -> String {
    const SPECIAL: &[char] = &[
        ':', '#', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ];
    if s.is_empty() || s.contains(SPECIAL) {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        s.to_string()
    }
}

/// Format a serde_json::Value for YAML-like output.
fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => yaml_escape(s),
                    other => other.to_string(),
                })
                .collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::String(s) => yaml_escape(s),
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
    use crate::parser::ExportEntry;

    fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
        ExportEntry::new(name.to_string(), start, end)
    }

    #[test]
    fn test_sidecar_output() {
        let metadata = Metadata {
            exports: vec![
                entry("createSession", 5, 20),
                entry("validateSession", 22, 45),
            ],
            imports: vec!["jwt".to_string(), "redis".to_string()],
            dependencies: vec!["./types".to_string(), "./config".to_string()],
            loc: 234,
        };

        let fm = Frontmatter::new("src/auth/session.ts".to_string(), metadata);
        let rendered = fm.render();

        assert!(rendered.contains("file: src/auth/session.ts"));
        assert!(rendered.contains("exports:"));
        assert!(rendered.contains("  createSession: [5, 20]"));
        assert!(rendered.contains("  validateSession: [22, 45]"));
        assert!(rendered.contains("imports: [jwt, redis]"));
        assert!(rendered.contains("dependencies: [./types, ./config]"));
        assert!(rendered.contains("loc: 234"));
        assert!(rendered.contains("modified:"));
        assert!(!rendered.contains("//"));
        assert!(!rendered.contains("# ---"));
        assert!(!rendered.contains("FMM ---"));
    }

    #[test]
    fn test_sidecar_with_version() {
        let metadata = Metadata {
            exports: vec![entry("foo", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata).with_version("v0.3");
        let rendered = fm.render();

        assert!(rendered.contains("fmm: v0.3"));
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "---");
        assert_eq!(lines[1], "file: test.ts");
        assert_eq!(lines[2], "fmm: v0.3");
    }

    #[test]
    fn test_sidecar_without_version() {
        let metadata = Metadata {
            exports: vec![entry("foo", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata);
        let rendered = fm.render();
        assert!(!rendered.contains("fmm:"));
    }

    #[test]
    fn test_sidecar_with_rust_custom_fields() {
        let metadata = Metadata {
            exports: vec![entry("MyStruct", 5, 15)],
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

        let fm = Frontmatter::new("src/lib.rs".to_string(), metadata)
            .with_version("v0.3")
            .with_custom_fields(Some("rust"), Some(&custom));

        let rendered = fm.render();
        assert!(rendered.contains("rust:"));
        assert!(rendered.contains("  derives: [Debug, Clone]"));
        assert!(rendered.contains("  unsafe_blocks: 3"));
    }

    #[test]
    fn test_sidecar_with_python_custom_fields() {
        let metadata = Metadata {
            exports: vec![entry("MyClass", 1, 30)],
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

        let fm = Frontmatter::new("app.py".to_string(), metadata)
            .with_version("v0.3")
            .with_custom_fields(Some("python"), Some(&custom));

        let rendered = fm.render();
        assert!(rendered.contains("python:"));
        assert!(rendered.contains("  decorators: [staticmethod, property]"));
    }

    #[test]
    fn test_sidecar_no_custom_fields_when_none() {
        let metadata = Metadata {
            exports: vec![entry("foo", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let fm = Frontmatter::new("test.ts".to_string(), metadata).with_custom_fields(None, None);

        let rendered = fm.render();
        assert!(!rendered.contains("typescript:"));
    }

    #[test]
    fn test_sidecar_no_custom_fields_when_empty() {
        let metadata = Metadata {
            exports: vec![entry("foo", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
        };

        let empty: HashMap<String, serde_json::Value> = HashMap::new();
        let fm = Frontmatter::new("test.ts".to_string(), metadata)
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

    #[test]
    fn yaml_escape_leaves_safe_strings_unquoted() {
        assert_eq!(yaml_escape("createSession"), "createSession");
        assert_eq!(yaml_escape("src/auth/session.ts"), "src/auth/session.ts");
        assert_eq!(yaml_escape("./types"), "./types");
        assert_eq!(yaml_escape("react-dom"), "react-dom");
    }

    #[test]
    fn yaml_escape_quotes_special_characters() {
        assert_eq!(yaml_escape("key:value"), "'key:value'");
        assert_eq!(yaml_escape("foo#bar"), "'foo#bar'");
        assert_eq!(yaml_escape("[array]"), "'[array]'");
        assert_eq!(yaml_escape("@angular/core"), "'@angular/core'");
        assert_eq!(yaml_escape(""), "''");
    }

    #[test]
    fn yaml_escape_handles_embedded_single_quotes() {
        assert_eq!(yaml_escape("it's:here"), "'it''s:here'");
    }

    #[test]
    fn render_starts_with_yaml_document_marker() {
        let metadata = Metadata {
            exports: vec![],
            imports: vec![],
            dependencies: vec![],
            loc: 1,
        };
        let fm = Frontmatter::new("test.ts".to_string(), metadata);
        assert!(fm.render().starts_with("---\n"));
    }

    #[test]
    fn special_chars_roundtrip_through_yaml() {
        let metadata = Metadata {
            exports: vec![entry("Config:Base", 1, 10)],
            imports: vec!["@scope/pkg".to_string()],
            dependencies: vec![],
            loc: 20,
        };
        let fm =
            Frontmatter::new("src/utils/key:value.ts".to_string(), metadata).with_version("v0.3");
        let rendered = fm.render();

        // Parse back through serde_yaml
        let parsed: serde_yaml::Value = serde_yaml::from_str(&rendered).unwrap();
        assert_eq!(parsed["file"].as_str().unwrap(), "src/utils/key:value.ts");
        assert_eq!(
            parsed["imports"].as_sequence().unwrap()[0]
                .as_str()
                .unwrap(),
            "@scope/pkg"
        );
        // Export names are map keys
        assert!(parsed["exports"]["Config:Base"].is_sequence());
    }
}
