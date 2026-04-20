use crate::support::parse_with;
use fmm_core::parser::builtin::ruby::RubyParser;

// Ruby validation

/// Rails-style ActiveRecord model
#[test]
fn ruby_real_repo_rails_model() {
    let source = include_str!("fixtures/ruby/ruby_real_repo_rails_model.rb");
    let result = parse_with(RubyParser::new().unwrap(), source);

    assert!(result.metadata.export_names().contains(&"User".to_string()));
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Searchable".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"create_user".to_string())
    );

    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"concerns/searchable".to_string())
    );

    let fields = result.custom_fields.unwrap();
    let mixins = fields.get("mixins").unwrap().as_array().unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Searchable"));
}

/// Ruby module with mixins and metaprogramming
#[test]
fn ruby_real_repo_module_mixins() {
    let source = include_str!("fixtures/ruby/ruby_real_repo_module_mixins.rb");
    let result = parse_with(RubyParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Loggable".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Configurable".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Application".to_string())
    );

    assert!(result.metadata.imports.contains(&"logger".to_string()));

    let fields = result.custom_fields.unwrap();
    let mixins = fields.get("mixins").unwrap().as_array().unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Loggable"));
    assert!(mixin_names.contains(&"Configurable"));
}
