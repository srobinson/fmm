use fmm_core::convention::{ConventionPlugin, ConventionRegistry, ConventionTestPatterns};
use fmm_core::parser::ParserRegistry;

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

struct ReplacementFixtureConvention;

impl ConventionPlugin for ReplacementFixtureConvention {
    fn id() -> &'static str {
        "fixture"
    }

    fn entry_patterns() -> &'static [&'static str] {
        &["src/replacement.ts"]
    }

    fn always_used_symbols() -> &'static [&'static str] {
        &["defineReplacement"]
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

#[test]
fn convention_registry_registers_plugins_by_type_without_parser_instances() {
    let parser_registry = ParserRegistry::new();
    let mut registry = ConventionRegistry::new(&parser_registry);

    assert!(registry.register::<FixtureConvention>().is_none());
    assert!(registry.register::<MinimalConvention>().is_none());

    let ids: Vec<_> = registry.plugins().map(|plugin| plugin.id).collect();
    assert_eq!(ids, vec!["fixture", "minimal"]);

    let fixture = registry.plugin("fixture").expect("fixture plugin");
    assert_eq!(fixture.languages, &["ts", "tsx"]);
    assert_eq!(fixture.enablers, &["fixture"]);
    assert_eq!(fixture.entry_patterns, &["src/main.ts"]);
    assert_eq!(fixture.generated_patterns, &["generated/**"]);
    assert_eq!(fixture.virtual_module_prefixes, &["virtual:fixture"]);
    assert_eq!(fixture.always_used_symbols, &["defineFixture"]);
    assert_eq!(fixture.test_patterns.filename_suffixes, &[".fixture.ts"]);
}

#[test]
fn convention_registry_duplicate_ids_overwrite_deterministically() {
    let parser_registry = ParserRegistry::new();
    let mut registry = ConventionRegistry::new(&parser_registry);

    assert!(registry.register::<FixtureConvention>().is_none());
    assert!(
        registry
            .register::<ReplacementFixtureConvention>()
            .is_some()
    );

    let plugins: Vec<_> = registry.plugins().collect();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, "fixture");
    assert_eq!(plugins[0].entry_patterns, &["src/replacement.ts"]);
    assert_eq!(plugins[0].always_used_symbols, &["defineReplacement"]);
}

#[test]
fn convention_registry_exposes_parser_descriptor_conventions_through_adapter() {
    let parser_registry = ParserRegistry::with_builtins();
    let registry = ConventionRegistry::new(&parser_registry);

    assert_eq!(
        registry.language_descriptors().len(),
        parser_registry.descriptors().len()
    );
    assert!(registry.source_extensions().contains("rs"));
    assert!(registry.is_reexport_file("__init__.py"));
    assert!(registry.is_language_test_file("src/lib_test.rs"));

    let symbol_prefixes: Vec<_> = registry
        .language_test_patterns()
        .flat_map(|patterns| patterns.test_symbol_prefixes.iter().copied())
        .collect();
    assert!(symbol_prefixes.contains(&"test_"));
}
