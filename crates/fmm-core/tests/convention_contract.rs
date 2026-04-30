use fmm_core::convention::{ConventionPlugin, ConventionTestPatterns};

struct FixtureConvention;

impl ConventionPlugin for FixtureConvention {
    fn id() -> &'static str {
        "fixture"
    }

    fn languages() -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn enablers() -> &'static [&'static str] {
        &["fixture"]
    }

    fn entry_patterns() -> &'static [&'static str] {
        &["src/main.ts"]
    }

    fn generated_patterns() -> &'static [&'static str] {
        &["generated/**"]
    }

    fn virtual_module_prefixes() -> &'static [&'static str] {
        &["virtual:fixture"]
    }

    fn always_used_symbols() -> &'static [&'static str] {
        &["defineFixture"]
    }

    fn test_patterns() -> ConventionTestPatterns {
        ConventionTestPatterns {
            path_contains: &["/fixtures/"],
            filename_suffixes: &[".fixture.ts"],
            filename_prefixes: &["fixture_"],
            test_symbol_prefixes: &["fixture_"],
        }
    }
}

struct MinimalConvention;

impl ConventionPlugin for MinimalConvention {
    fn id() -> &'static str {
        "minimal"
    }
}

#[test]
fn convention_plugin_static_accessors_are_available_without_parser_instances() {
    assert_eq!(FixtureConvention::id(), "fixture");
    assert_eq!(FixtureConvention::languages(), &["ts", "tsx"]);
    assert_eq!(FixtureConvention::enablers(), &["fixture"]);
    assert_eq!(FixtureConvention::entry_patterns(), &["src/main.ts"]);
    assert_eq!(FixtureConvention::generated_patterns(), &["generated/**"]);
    assert_eq!(
        FixtureConvention::virtual_module_prefixes(),
        &["virtual:fixture"]
    );
    assert_eq!(FixtureConvention::always_used_symbols(), &["defineFixture"]);

    let test_patterns = FixtureConvention::test_patterns();
    assert_eq!(test_patterns.path_contains, &["/fixtures/"]);
    assert_eq!(test_patterns.filename_suffixes, &[".fixture.ts"]);
    assert_eq!(test_patterns.filename_prefixes, &["fixture_"]);
    assert_eq!(test_patterns.test_symbol_prefixes, &["fixture_"]);
}

#[test]
fn convention_plugin_defaults_are_empty_static_slices() {
    assert_eq!(MinimalConvention::id(), "minimal");
    assert_eq!(MinimalConvention::languages(), &[] as &[&str]);
    assert_eq!(MinimalConvention::enablers(), &[] as &[&str]);
    assert_eq!(MinimalConvention::entry_patterns(), &[] as &[&str]);
    assert_eq!(MinimalConvention::generated_patterns(), &[] as &[&str]);
    assert_eq!(MinimalConvention::virtual_module_prefixes(), &[] as &[&str]);
    assert_eq!(MinimalConvention::always_used_symbols(), &[] as &[&str]);

    let test_patterns = MinimalConvention::test_patterns();
    assert_eq!(test_patterns.path_contains, &[] as &[&str]);
    assert_eq!(test_patterns.filename_suffixes, &[] as &[&str]);
    assert_eq!(test_patterns.filename_prefixes, &[] as &[&str]);
    assert_eq!(test_patterns.test_symbol_prefixes, &[] as &[&str]);
}

#[test]
fn convention_plugin_contract_remains_registerable_by_type() {
    fn assert_contract<T: ConventionPlugin + Send + Sync + 'static>() {}

    assert_contract::<FixtureConvention>();
    assert_contract::<MinimalConvention>();
}
