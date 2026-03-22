use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

use crate::parser::ParserRegistry;

/// Patterns used to classify test vs. source files.
///
/// A file is classified as a test if its path contains any `path_contains`
/// segment or its filename ends with any `filename_suffixes` entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPatterns {
    /// Path segments that indicate a test file (e.g. "/test/", "/e2e/")
    #[serde(default = "default_test_path_contains")]
    pub path_contains: Vec<String>,
    /// Filename suffix patterns that indicate a test file (e.g. ".spec.ts")
    #[serde(default = "default_test_filename_suffixes")]
    pub filename_suffixes: Vec<String>,
}

impl Default for TestPatterns {
    fn default() -> Self {
        Self {
            path_contains: default_test_path_contains(),
            filename_suffixes: default_test_filename_suffixes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Languages to process
    #[serde(default = "default_languages")]
    pub languages: BTreeSet<String>,
    /// Patterns for detecting test files (used by fmm_list_files filter parameter)
    #[serde(default)]
    pub test_patterns: TestPatterns,
    /// Maximum number of lines per file. Files exceeding this limit are skipped during indexing.
    /// Default: 100,000. Set to 0 to disable the limit.
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
    /// Glob patterns (relative to project root) to exclude from indexing,
    /// in addition to .gitignore and .fmmignore rules.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            languages: default_languages(),
            test_patterns: TestPatterns::default(),
            max_lines: default_max_lines(),
            exclude: Vec::new(),
        }
    }
}

impl Config {
    /// Construct a default `Config` whose `languages` are derived from the registry.
    ///
    /// Preferred over `Config::default()` when a `ParserRegistry` is already
    /// available: the language set is always in sync with registered parsers
    /// without touching the hardcoded `default_languages()` fallback.
    pub fn default_with_registry(registry: &ParserRegistry) -> Self {
        Self {
            languages: registry.source_extensions().iter().cloned().collect(),
            ..Default::default()
        }
    }

    pub fn load() -> Result<Self> {
        Self::load_from_dir(Path::new("."))
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let toml_path = dir.join(".fmmrc.toml");
        if toml_path.exists() {
            let content = std::fs::read_to_string(&toml_path)
                .with_context(|| format!("Failed to read {}", toml_path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", toml_path.display()))?;
            return Ok(config);
        }

        Ok(Self::default())
    }

    pub fn is_supported_language(&self, extension: &str) -> bool {
        self.languages.contains(extension)
    }

    /// Return true if `path` matches the configured test-file heuristics.
    pub fn is_test_file(&self, path: &str) -> bool {
        let tp = &self.test_patterns;
        // Check path-segment patterns (use `/` prefix to avoid matching partial names)
        for seg in &tp.path_contains {
            if path.contains(seg.as_str()) {
                return true;
            }
        }
        // Check filename suffix patterns
        let filename = path.rsplit('/').next().unwrap_or(path);
        for suffix in &tp.filename_suffixes {
            if filename.ends_with(suffix.as_str()) {
                return true;
            }
        }
        false
    }
}

fn default_max_lines() -> usize {
    100_000
}

fn default_test_path_contains() -> Vec<String> {
    vec![
        "/e2e/".to_string(),
        "/test/".to_string(),
        "/tests/".to_string(),
        "/spec/".to_string(),
        "/__tests__/".to_string(),
    ]
}

fn default_test_filename_suffixes() -> Vec<String> {
    vec![
        ".spec.ts".to_string(),
        ".test.ts".to_string(),
        ".e2e-spec.ts".to_string(),
        ".spec.js".to_string(),
        ".test.js".to_string(),
        "_test.go".to_string(),
        "_test.rs".to_string(),
        ".spec.tsx".to_string(),
        ".test.tsx".to_string(),
    ]
}

fn default_languages() -> BTreeSet<String> {
    [
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh", "cxx", "hxx",
        "cs", "rb", "php", "c", "h", "zig", "lua", "scala", "sc", "swift", "kt", "kts", "dart",
        "ex", "exs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParserRegistry;
    use std::fs;
    use tempfile::TempDir;

    /// Guard: the hardcoded `default_languages()` fallback must stay in sync with
    /// the extensions reported by all registered builtin parsers.
    ///
    /// If this test fails, update `default_languages()` to match the registry.
    #[test]
    fn default_languages_matches_registry() {
        let registry = ParserRegistry::with_builtins();
        let from_registry: BTreeSet<String> =
            registry.source_extensions().iter().cloned().collect();
        let from_hardcoded = default_languages();
        assert_eq!(
            from_registry, from_hardcoded,
            "default_languages() is out of sync with ParserRegistry — update one or the other"
        );
    }

    /// Registry-derived config contains exactly the registered extensions.
    #[test]
    fn default_with_registry_uses_registry_extensions() {
        let registry = ParserRegistry::with_builtins();
        let config = Config::default_with_registry(&registry);
        let expected: BTreeSet<String> = registry.source_extensions().iter().cloned().collect();
        assert_eq!(config.languages, expected);
    }

    #[test]
    fn default_config_has_expected_languages() {
        let config = Config::default();
        for ext in [
            "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh", "cxx",
            "hxx", "cs", "rb", "php", "c", "h", "zig", "lua", "scala", "sc", "swift", "kt", "kts",
            "dart", "ex", "exs",
        ] {
            assert!(config.languages.contains(ext), "missing extension: {ext}");
        }
        assert_eq!(config.languages.len(), 29);
    }

    #[test]
    fn returns_default_when_no_config_file() {
        let tmp = TempDir::new().unwrap();
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 29);
    }

    #[test]
    fn json_config_file_is_not_loaded() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".fmmrc.json"),
            r#"{ "languages": ["rs", "py"] }"#,
        )
        .unwrap();

        // .fmmrc.json is no longer loaded; should return defaults
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 29);
    }

    #[test]
    fn empty_languages_list() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.toml"), "languages = []\n").unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert!(config.languages.is_empty());
    }

    #[test]
    fn unknown_language_extension_accepted() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".fmmrc.toml"),
            "languages = [\"xyz\", \"abc\"]\n",
        )
        .unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 2);
        assert!(config.languages.contains("xyz"));
    }

    #[test]
    fn is_test_file_detects_spec_suffix() {
        let config = Config::default();
        assert!(config.is_test_file("src/auth/auth.spec.ts"));
        assert!(config.is_test_file("src/core/core.test.ts"));
        assert!(config.is_test_file("src/auth/auth.e2e-spec.ts"));
        assert!(!config.is_test_file("src/auth/auth.service.ts"));
        assert!(!config.is_test_file("src/core/index.ts"));
    }

    #[test]
    fn is_test_file_detects_path_segment() {
        let config = Config::default();
        assert!(config.is_test_file("src/test/helper.ts"));
        assert!(config.is_test_file("packages/core/e2e/app.ts"));
        assert!(config.is_test_file("src/__tests__/utils.ts"));
        assert!(!config.is_test_file("src/contest/result.ts")); // "contest" does not match "/test/"
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
        assert!(config.is_supported_language("php"));
        assert!(config.is_supported_language("c"));
        assert!(config.is_supported_language("h"));
        assert!(config.is_supported_language("zig"));
        assert!(config.is_supported_language("lua"));
        assert!(config.is_supported_language("scala"));
        assert!(config.is_supported_language("sc"));
        assert!(config.is_supported_language("swift"));
        assert!(config.is_supported_language("kt"));
        assert!(config.is_supported_language("kts"));
        assert!(config.is_supported_language("dart"));
        assert!(config.is_supported_language("ex"));
        assert!(config.is_supported_language("exs"));
        assert!(!config.is_supported_language(""));
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.languages, deserialized.languages);
    }

    // max_lines and exclude tests

    #[test]
    fn default_max_lines_is_100k() {
        let config = Config::default();
        assert_eq!(config.max_lines, 100_000);
    }

    #[test]
    fn default_exclude_is_empty() {
        let config = Config::default();
        assert!(config.exclude.is_empty());
    }

    #[test]
    fn loads_max_lines_from_toml() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.toml"), "max_lines = 50000\n").unwrap();
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.max_lines, 50_000);
    }

    #[test]
    fn loads_exclude_from_toml() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".fmmrc.toml"),
            r#"exclude = ["vendor/**", "benchmarks/fixtures/**"]"#,
        )
        .unwrap();
        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.exclude.len(), 2);
        assert_eq!(config.exclude[0], "vendor/**");
        assert_eq!(config.exclude[1], "benchmarks/fixtures/**");
    }

    // TOML loading tests

    #[test]
    fn loads_toml_config_with_languages() {
        let tmp = TempDir::new().unwrap();
        let toml = r#"languages = ["rs", "py"]"#;
        fs::write(tmp.path().join(".fmmrc.toml"), toml).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 2);
        assert!(config.languages.contains("rs"));
        assert!(config.languages.contains("py"));
    }

    #[test]
    fn handles_invalid_toml_as_error() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.toml"), "not = toml = at = all %%%").unwrap();
        let result = Config::load_from_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn empty_toml_gives_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".fmmrc.toml"), "").unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert_eq!(config.languages.len(), 29);
    }

    #[test]
    fn toml_test_patterns_configurable() {
        let tmp = TempDir::new().unwrap();
        let toml = r#"
[test_patterns]
path_contains = ["/custom_tests/"]
filename_suffixes = [".myspec.ts"]
"#;
        fs::write(tmp.path().join(".fmmrc.toml"), toml).unwrap();

        let config = Config::load_from_dir(tmp.path()).unwrap();
        assert!(config.is_test_file("src/custom_tests/foo.ts"));
        assert!(config.is_test_file("src/bar.myspec.ts"));
        assert!(!config.is_test_file("src/auth.spec.ts"));
        assert!(!config.is_test_file("src/test/foo.ts"));
    }
}
