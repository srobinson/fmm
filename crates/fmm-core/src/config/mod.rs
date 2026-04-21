use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use tracing::warn;

use crate::parser::ParserRegistry;

mod defaults;
mod loader;
#[cfg(test)]
mod tests;

use defaults::{
    default_languages, default_max_lines, default_test_filename_suffixes,
    default_test_path_contains,
};

/// Patterns used to classify test vs. source files.
///
/// A file is classified as a test if its path contains any `path_contains`
/// segment or its filename ends with any `filename_suffixes` entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPatterns {
    /// Path segments that indicate a test file, for example "/test/" or "/e2e/".
    #[serde(default = "default_test_path_contains")]
    pub path_contains: Vec<String>,
    /// Filename suffix patterns that indicate a test file, for example ".spec.ts".
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
    /// Languages to process.
    #[serde(default = "default_languages")]
    pub languages: BTreeSet<String>,
    /// Patterns for detecting test files.
    #[serde(default)]
    pub test_patterns: TestPatterns,
    /// Maximum number of lines per file. Files exceeding this limit are skipped during indexing.
    /// Default: 100,000. Set to 0 to disable the limit.
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
    /// Glob patterns relative to project root to exclude from indexing, in addition
    /// to .gitignore and .fmmignore rules.
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
        let mut config = Self::default();

        let toml_path = dir.join(".fmmrc.toml");
        if toml_path.exists() {
            match std::fs::read_to_string(&toml_path) {
                Ok(content) => match toml::from_str::<loader::FileConfig>(&content) {
                    Ok(file_config) => loader::apply_file_config(&mut config, file_config),
                    Err(e) => {
                        warn!(
                            path = %toml_path.display(),
                            error = %e,
                            "failed to parse config; using defaults"
                        );
                        config = Self::default();
                    }
                },
                Err(e) => {
                    warn!(
                        path = %toml_path.display(),
                        error = %e,
                        "failed to read config; using defaults"
                    );
                    config = Self::default();
                }
            }
        }

        loader::apply_env_overrides(&mut config);

        if let Err(msg) = config.validate() {
            warn!(reason = %msg, "config validation failed; falling back to defaults");
            return Ok(Self::default());
        }

        Ok(config)
    }

    /// Check invariants after merging file config and env overrides.
    /// Returns `Err(reason)` if the config is invalid; the caller warns and
    /// falls back to `Config::default()`.
    fn validate(&self) -> Result<(), String> {
        if self.languages.is_empty() {
            return Err("languages must not be empty when explicitly set".into());
        }
        Ok(())
    }

    pub fn is_supported_language(&self, extension: &str) -> bool {
        self.languages.contains(extension)
    }

    /// Return true if `path` matches the configured test-file heuristics.
    pub fn is_test_file(&self, path: &str) -> bool {
        let tp = &self.test_patterns;
        for seg in &tp.path_contains {
            if path.contains(seg.as_str()) {
                return true;
            }
        }

        let filename = path.rsplit('/').next().unwrap_or(path);
        for suffix in &tp.filename_suffixes {
            if filename.ends_with(suffix.as_str()) {
                return true;
            }
        }

        false
    }
}
