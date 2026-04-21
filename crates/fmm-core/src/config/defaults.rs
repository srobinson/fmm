use std::collections::BTreeSet;

pub(super) fn default_max_lines() -> usize {
    100_000
}

pub(super) fn default_test_path_contains() -> Vec<String> {
    vec![
        "/e2e/".to_string(),
        "/test/".to_string(),
        "/tests/".to_string(),
        "/spec/".to_string(),
        "/__tests__/".to_string(),
    ]
}

pub(super) fn default_test_filename_suffixes() -> Vec<String> {
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

pub(super) fn default_languages() -> BTreeSet<String> {
    [
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "hpp", "cc", "hh", "cxx", "hxx",
        "cs", "rb", "php", "c", "h", "zig", "lua", "scala", "sc", "swift", "kt", "kts", "dart",
        "ex", "exs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
