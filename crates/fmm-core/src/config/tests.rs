use super::defaults::default_languages;
use super::*;
use crate::parser::ParserRegistry;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

const FMM_ENV_KEYS: [&str; 3] = ["FMM_MAX_LINES", "FMM_LANGUAGES", "FMM_EXCLUDE"];
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn new() -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = FMM_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();

        for key in FMM_ENV_KEYS {
            // SAFETY: config tests hold ENV_LOCK while mutating process env.
            unsafe { std::env::remove_var(key) };
        }

        Self { _lock: lock, saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for key in FMM_ENV_KEYS {
            // SAFETY: config tests hold ENV_LOCK while mutating process env.
            unsafe { std::env::remove_var(key) };
        }
        for (key, value) in &self.saved {
            if let Some(value) = value {
                // SAFETY: config tests hold ENV_LOCK while mutating process env.
                unsafe { std::env::set_var(key, value) };
            }
        }
    }
}

fn load_clean_from_dir(dir: &Path) -> Config {
    let _env = EnvGuard::new();
    Config::load_from_dir(dir).unwrap()
}

/// Guard: the hardcoded `default_languages()` fallback must stay in sync with
/// the extensions reported by all registered builtin parsers.
///
/// If this test fails, update `default_languages()` to match the registry.
#[test]
fn default_languages_matches_registry() {
    let registry = ParserRegistry::with_builtins();
    let from_registry: BTreeSet<String> = registry.source_extensions().iter().cloned().collect();
    let from_hardcoded = default_languages();
    assert_eq!(
        from_registry, from_hardcoded,
        "default_languages() is out of sync with ParserRegistry: update one or the other"
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
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh", "cxx", "hxx",
        "cs", "rb", "php", "c", "h", "zig", "lua", "scala", "sc", "swift", "kt", "kts", "dart",
        "ex", "exs",
    ] {
        assert!(config.languages.contains(ext), "missing extension: {ext}");
    }
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn returns_default_when_no_config_file() {
    let tmp = TempDir::new().unwrap();
    let config = load_clean_from_dir(tmp.path());
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

    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn empty_languages_falls_back_to_defaults() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "languages = []\n").unwrap();

    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn unknown_language_extension_accepted() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join(".fmmrc.toml"),
        "languages = [\"xyz\", \"abc\"]\n",
    )
    .unwrap();

    let config = load_clean_from_dir(tmp.path());
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
    assert!(!config.is_test_file("src/contest/result.ts"));
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
    let config = load_clean_from_dir(tmp.path());
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
    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.exclude.len(), 2);
    assert_eq!(config.exclude[0], "vendor/**");
    assert_eq!(config.exclude[1], "benchmarks/fixtures/**");
}

#[test]
fn loads_toml_config_with_languages() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"languages = ["rs", "py"]"#;
    fs::write(tmp.path().join(".fmmrc.toml"), toml).unwrap();

    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.languages.len(), 2);
    assert!(config.languages.contains("rs"));
    assert!(config.languages.contains("py"));
}

#[test]
fn unknown_keys_fall_back_to_defaults() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join(".fmmrc.toml"),
        "languages = [\"rs\"]\nbogus_key = true\n",
    )
    .unwrap();
    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn malformed_toml_falls_back_to_defaults() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "not = toml = at = all %%%").unwrap();
    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn empty_toml_gives_defaults() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "").unwrap();

    let config = load_clean_from_dir(tmp.path());
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

    let config = load_clean_from_dir(tmp.path());
    assert!(config.is_test_file("src/custom_tests/foo.ts"));
    assert!(config.is_test_file("src/bar.myspec.ts"));
    assert!(!config.is_test_file("src/auth.spec.ts"));
    assert!(!config.is_test_file("src/test/foo.ts"));
}

#[test]
fn env_fmm_max_lines_overrides_toml() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "max_lines = 5000\n").unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_MAX_LINES", "999") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.max_lines, 999);
}

#[test]
fn env_fmm_max_lines_overrides_default() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_MAX_LINES", "42") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.max_lines, 42);
}

#[test]
fn env_fmm_max_lines_invalid_keeps_current() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "max_lines = 5000\n").unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_MAX_LINES", "not_a_number") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.max_lines, 5000);
}

#[test]
fn env_fmm_languages_overrides_toml() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join(".fmmrc.toml"),
        r#"languages = ["rs", "py"]"#,
    )
    .unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_LANGUAGES", "go, java, kt") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.languages.len(), 3);
    assert!(config.languages.contains("go"));
    assert!(config.languages.contains("java"));
    assert!(config.languages.contains("kt"));
}

#[test]
fn env_fmm_languages_trims_whitespace() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_LANGUAGES", "  rs , py  ") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.languages.len(), 2);
    assert!(config.languages.contains("rs"));
    assert!(config.languages.contains("py"));
}

#[test]
fn env_fmm_exclude_overrides_toml() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), r#"exclude = ["vendor/**"]"#).unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_EXCLUDE", "dist/**, build/**") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.exclude.len(), 2);
    assert_eq!(config.exclude[0], "dist/**");
    assert_eq!(config.exclude[1], "build/**");
}

#[test]
fn env_vars_applied_without_toml_file() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe {
        std::env::set_var("FMM_MAX_LINES", "777");
        std::env::set_var("FMM_LANGUAGES", "zig,lua");
        std::env::set_var("FMM_EXCLUDE", "tmp/**");
    }
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.max_lines, 777);
    assert_eq!(config.languages.len(), 2);
    assert!(config.languages.contains("zig"));
    assert!(config.languages.contains("lua"));
    assert_eq!(config.exclude, vec!["tmp/**"]);
}

#[test]
fn file_config_partial_deserialization() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "max_lines = 42\n").unwrap();
    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.max_lines, 42);
    assert_eq!(config.languages.len(), 29);
    assert!(config.exclude.is_empty());
    assert_eq!(config.test_patterns.path_contains.len(), 5);
}

#[test]
fn three_layer_precedence() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join(".fmmrc.toml"),
        "max_lines = 5000\nlanguages = [\"rs\", \"py\"]\n",
    )
    .unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe {
        std::env::set_var("FMM_MAX_LINES", "123");
        std::env::set_var("FMM_EXCLUDE", "dist/**");
    }
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.max_lines, 123);
    assert_eq!(config.languages.len(), 2);
    assert!(config.languages.contains("rs"));
    assert_eq!(config.exclude, vec!["dist/**"]);
    assert_eq!(config.test_patterns.path_contains.len(), 5);
}

#[test]
fn env_fmm_languages_empty_string_falls_back_to_defaults() {
    let _env = EnvGuard::new();
    let tmp = TempDir::new().unwrap();
    // SAFETY: EnvGuard serializes config tests and restores process env.
    unsafe { std::env::set_var("FMM_LANGUAGES", "") };
    let config = Config::load_from_dir(tmp.path()).unwrap();
    assert_eq!(config.languages.len(), 29);
}

#[test]
fn max_lines_zero_is_valid() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".fmmrc.toml"), "max_lines = 0\n").unwrap();
    let config = load_clean_from_dir(tmp.path());
    assert_eq!(config.max_lines, 0);
}
