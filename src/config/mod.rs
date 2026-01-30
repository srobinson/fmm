use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Languages to process
    #[serde(default = "default_languages")]
    pub languages: HashSet<String>,

    /// Format for frontmatter
    #[serde(default = "default_format")]
    pub format: FrontmatterFormat,

    /// Include LOC (lines of code)
    #[serde(default = "default_true")]
    pub include_loc: bool,

    /// Include complexity metrics
    #[serde(default)]
    pub include_complexity: bool,

    /// Maximum file size to process (in KB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontmatterFormat {
    Yaml,
    Json,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            languages: default_languages(),
            format: default_format(),
            include_loc: true,
            include_complexity: false,
            max_file_size: 1024, // 1MB
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
    ["ts", "tsx", "js", "jsx", "py", "rs", "go"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_format() -> FrontmatterFormat {
    FrontmatterFormat::Yaml
}

fn default_true() -> bool {
    true
}

fn default_max_file_size() -> usize {
    1024 // 1MB in KB
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_expected_languages() {
        let config = Config::default();
        assert!(config.languages.contains("ts"));
        assert!(config.languages.contains("tsx"));
        assert!(config.languages.contains("js"));
        assert!(config.languages.contains("jsx"));
        assert!(config.languages.contains("py"));
        assert!(config.languages.contains("rs"));
        assert!(config.languages.contains("go"));
        assert_eq!(config.languages.len(), 7);
    }

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert!(config.include_loc);
        assert!(!config.include_complexity);
        assert_eq!(config.max_file_size, 1024);
        assert!(matches!(config.format, FrontmatterFormat::Yaml));
    }

    #[test]
    fn returns_default_when_no_config_file() {
        let tmp = TempDir::new().unwrap();
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 7);
        assert!(config.include_loc);
        assert_eq!(config.max_file_size, 1024);
    }

    #[test]
    fn loads_valid_full_config() {
        let tmp = TempDir::new().unwrap();
        let json = r#"{
            "languages": ["rs", "py"],
            "format": "json",
            "include_loc": false,
            "include_complexity": true,
            "max_file_size": 512
        }"#;
        fs::write(tmp.path().join(".fmmrc.json"), json).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 2);
        assert!(config.languages.contains("rs"));
        assert!(config.languages.contains("py"));
        assert!(matches!(config.format, FrontmatterFormat::Json));
        assert!(!config.include_loc);
        assert!(config.include_complexity);
        assert_eq!(config.max_file_size, 512);
    }

    #[test]
    fn handles_partial_config_with_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), r#"{ "languages": ["go"] }"#).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 1);
        assert!(config.languages.contains("go"));
        assert!(config.include_loc);
        assert!(!config.include_complexity);
        assert_eq!(config.max_file_size, 1024);
        assert!(matches!(config.format, FrontmatterFormat::Yaml));
    }

    #[test]
    fn handles_invalid_json_as_error() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), "not json at all {{{").unwrap();
        let result = Config::load_from_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn unknown_fields_ignored() {
        let tmp = TempDir::new().unwrap();
        let json = r#"{
            "languages": ["ts"],
            "totally_unknown_field": true,
            "another_one": 42
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
    fn max_file_size_zero() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), r#"{ "max_file_size": 0 }"#).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.max_file_size, 0);
    }

    #[test]
    fn is_supported_language_checks_membership() {
        let config = Config::default();
        assert!(config.is_supported_language("ts"));
        assert!(config.is_supported_language("py"));
        assert!(config.is_supported_language("rs"));
        assert!(!config.is_supported_language("cpp"));
        assert!(!config.is_supported_language("java"));
        assert!(!config.is_supported_language(""));
    }

    #[test]
    fn empty_json_object_gives_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.json"), "{}").unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 7);
        assert!(config.include_loc);
        assert_eq!(config.max_file_size, 1024);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.languages, deserialized.languages);
        assert_eq!(config.include_loc, deserialized.include_loc);
        assert_eq!(config.include_complexity, deserialized.include_complexity);
        assert_eq!(config.max_file_size, deserialized.max_file_size);
    }
}
