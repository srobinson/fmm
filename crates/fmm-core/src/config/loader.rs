use serde::Deserialize;
use std::collections::BTreeSet;
use tracing::warn;

use super::Config;

/// Intermediate deserialization target for `.fmmrc.toml`.
///
/// All fields are `Option` so partial configs are valid.
/// `deny_unknown_fields` catches typos and stale keys at deserialization time.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct FileConfig {
    languages: Option<BTreeSet<String>>,
    test_patterns: Option<FileTestPatterns>,
    max_lines: Option<usize>,
    exclude: Option<Vec<String>>,
}

/// Intermediate deserialization target for the `[test_patterns]` TOML section.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileTestPatterns {
    path_contains: Option<Vec<String>>,
    filename_suffixes: Option<Vec<String>>,
}

pub(super) fn apply_file_config(config: &mut Config, file_config: FileConfig) {
    if let Some(languages) = file_config.languages {
        config.languages = languages;
    }
    if let Some(max_lines) = file_config.max_lines {
        config.max_lines = max_lines;
    }
    if let Some(exclude) = file_config.exclude {
        config.exclude = exclude;
    }
    if let Some(test_patterns) = file_config.test_patterns {
        if let Some(path_contains) = test_patterns.path_contains {
            config.test_patterns.path_contains = path_contains;
        }
        if let Some(filename_suffixes) = test_patterns.filename_suffixes {
            config.test_patterns.filename_suffixes = filename_suffixes;
        }
    }
}

pub(super) fn apply_env_overrides(config: &mut Config) {
    if let Ok(val) = std::env::var("FMM_MAX_LINES") {
        match val.parse::<usize>() {
            Ok(n) => config.max_lines = n,
            Err(_) => {
                warn!(
                    var = "FMM_MAX_LINES",
                    value = %val,
                    "not a valid usize; keeping current value"
                );
            }
        }
    }
    if let Ok(val) = std::env::var("FMM_LANGUAGES") {
        config.languages = comma_separated_values(&val).collect();
    }
    if let Ok(val) = std::env::var("FMM_EXCLUDE") {
        config.exclude = comma_separated_values(&val).collect();
    }
}

fn comma_separated_values(val: &str) -> impl Iterator<Item = String> + '_ {
    val.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
