use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Languages to process
    #[serde(default = "default_languages")]
    pub languages: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            languages: default_languages(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from_dir(Path::new("."))
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join(".fmmrc.json");
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn is_supported_language(&self, extension: &str) -> bool {
        self.languages.contains(extension)
    }
}

fn default_languages() -> HashSet<String> {
    [
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh", "cxx",
        "hxx", "cs", "rb",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_expected_languages() {
        let config = Config::default();
        for ext in [
            "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh",
            "cxx", "hxx", "cs", "rb",
        ] {
            assert!(config.languages.contains(ext), "missing extension: {ext}");
        }
        assert_eq!(config.languages.len(), 16);
    }

    #[test]
    fn returns_default_when_no_config_file() {
        let tmp = TempDir::new().unwrap();
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 16);
    }

    #[test]
    fn loads_config_with_languages() {
        let tmp = TempDir::new().unwrap();
        let json = r#"{ "languages": ["rs", "py"] }"#;
        fs::write(tmp.path().join(".fmmrc.json"), json).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 2);
        assert!(config.languages.contains("rs"));
        assert!(config.languages.contains("py"));
    }

    #[test]
    fn handles_partial_config_with_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), r#"{ "languages": ["go"] }"#).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 1);
        assert!(config.languages.contains("go"));
    }

    #[test]
    fn handles_invalid_json_as_error() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), "not json at all {{{").unwrap();
        let result = Config::load_from_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn old_config_fields_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let json = r#"{
            "languages": ["ts"],
            "format": "json",
            "include_loc": false,
            "include_complexity": true,
            "max_file_size": 512,
            "totally_unknown_field": true
        }"#;
        fs::write(tmp.path().join(".fmmrc.json"), json).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 1);
        assert!(config.languages.contains("ts"));
    }

    #[test]
    fn empty_languages_list() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), r#"{ "languages": [] }"#).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert!(config.languages.is_empty());
    }

    #[test]
    fn unknown_language_extension_accepted() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".fmmrc.json"),
            r#"{ "languages": ["xyz", "abc"] }"#,
        )
        .unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 2);
        assert!(config.languages.contains("xyz"));
    }

    #[test]
    fn is_supported_language_checks_membership() {
        let config = Config::default();
        assert!(config.is_supported_language("ts"));
        assert!(config.is_supported_language("py"));
        assert!(config.is_supported_language("rs"));
        assert!(config.is_supported_language("cpp"));
        assert!(config.is_supported_language("java"));
        assert!(config.is_supported_language("rb"));
        assert!(config.is_supported_language("cs"));
        assert!(!config.is_supported_language("zig"));
        assert!(!config.is_supported_language(""));
    }

    #[test]
    fn empty_json_object_gives_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), "{}").unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 16);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.languages, deserialized.languages);
    }
}
