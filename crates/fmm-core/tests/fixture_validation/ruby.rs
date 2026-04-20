use super::*;

#[test]
fn validate_ruby_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.rb"
    ));

    let result = parse_fixture(RubyParser::new().unwrap(), source);

    // Classes, modules, top-level methods
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"DataProcessor".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ProcessConfig".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Cacheable".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"transform".to_string())
    );

    // Private methods excluded
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"_internal_helper".to_string())
    );

    // Imports (require)
    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(result.metadata.imports.contains(&"net/http".to_string()));

    // Dependencies (require_relative)
    assert!(result.metadata.dependencies.contains(&"config".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"lib/helpers".to_string())
    );

    // Mixins (custom fields)
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let mixins = fields
        .get("mixins")
        .expect("should have mixins")
        .as_array()
        .unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Comparable"));
    assert!(mixin_names.contains(&"Enumerable"));

    assert!(result.metadata.loc > 50);
}
