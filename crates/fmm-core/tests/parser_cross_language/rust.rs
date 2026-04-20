use crate::support::{
    assert_dependencies_include, assert_exports_exclude, assert_exports_include,
    assert_imports_include, custom_string_array, custom_u64, parse_with,
};
use fmm_core::parser::builtin::rust::RustParser;

// Rust validation

/// Rust module with pub structs, derives, and trait impls (bat-style)
#[test]
fn rust_real_repo_bat_style_config() {
    let source = include_str!("fixtures/rust/rust_real_repo_bat_style_config.rs");
    let result = parse_with(RustParser::new().unwrap(), source);

    assert_exports_include(&result, &["Config", "PagingMode", "load_config"]);
    assert_eq!(result.metadata.export_names().len(), 3);

    assert_imports_include(&result, &["anyhow", "serde"]);

    let derive_names = custom_string_array(&result, "derives");
    for derive in [
        "Debug",
        "Clone",
        "Serialize",
        "Deserialize",
        "PartialEq",
        "Eq",
        "Copy",
    ] {
        assert!(derive_names.iter().any(|name| name == derive));
    }

    let impl_names = custom_string_array(&result, "trait_impls");
    assert!(impl_names.iter().any(|name| name == "Default for Config"));
    assert!(
        impl_names
            .iter()
            .any(|name| name == "Default for PagingMode")
    );
}

/// Rust module with lifetimes, unsafe, async (ripgrep-style searcher)
#[test]
fn rust_real_repo_ripgrep_style_searcher() {
    let source = include_str!("fixtures/rust/rust_real_repo_ripgrep_style_searcher.rs");
    let result = parse_with(RustParser::new().unwrap(), source);

    assert_exports_include(&result, &["Searcher", "Match", "search_file"]);
    assert_imports_include(&result, &["anyhow", "tokio"]);
    assert_dependencies_include(&result, &["crate::config", "../matcher"]);

    assert_eq!(custom_u64(&result, "unsafe_blocks"), 1);
    assert_eq!(custom_u64(&result, "async_functions"), 1);

    let lifetime_names = custom_string_array(&result, "lifetimes");
    assert!(lifetime_names.iter().any(|name| name == "'a"));
}

/// Rust module with pub(crate) and pub(super) — should be excluded from exports
#[test]
fn rust_real_repo_visibility_filtering() {
    let source = include_str!("fixtures/rust/rust_real_repo_visibility_filtering.rs");
    let result = parse_with(RustParser::new().unwrap(), source);

    assert_exports_include(&result, &["public_api", "PublicType"]);
    assert_exports_exclude(
        &result,
        &["internal_helper", "parent_only", "totally_private"],
    );
    assert_eq!(result.metadata.export_names().len(), 2);
}

/// Rust module with multiple derive blocks and use groups
#[test]
fn rust_real_repo_complex_derives_and_use_groups() {
    let source = include_str!("fixtures/rust/rust_real_repo_complex_derives_and_use_groups.rs");
    let result = parse_with(RustParser::new().unwrap(), source);

    assert_exports_include(&result, &["AppState", "CacheEntry", "CacheError"]);
    assert_imports_include(&result, &["anyhow", "serde", "tokio"]);

    let derive_names = custom_string_array(&result, "derives");
    for derive in ["Debug", "Clone", "Serialize", "Deserialize"] {
        assert!(derive_names.iter().any(|name| name == derive));
    }

    let impl_names = custom_string_array(&result, "trait_impls");
    assert!(
        impl_names
            .iter()
            .any(|name| name == "Display for CacheError")
    );
}
