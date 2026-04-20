use crate::support::parse_with;
use fmm_core::parser::builtin::php::PhpParser;

// PHP validation

/// Laravel controller pattern with dependency injection and middleware
#[test]
fn php_real_repo_laravel_controller() {
    let source = include_str!("fixtures/php/php_real_repo_laravel_controller.php");
    let result = parse_with(PhpParser::new().unwrap(), source);

    // Class exported
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"PostController".to_string())
    );
    // Public methods exported
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"index".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"store".to_string())
    );
    assert!(result.metadata.export_names().contains(&"show".to_string()));
    // Private method NOT exported
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"authorize".to_string())
    );

    // Namespace imports
    assert!(result.metadata.imports.contains(&"App".to_string()));
    assert!(result.metadata.imports.contains(&"Illuminate".to_string()));

    // Namespace custom field
    let fields = result.custom_fields.unwrap();
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    let ns: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns.iter().any(|n| n.contains("Controllers")));
}

/// Composer package pattern with interfaces and traits
#[test]
fn php_real_repo_composer_package() {
    let source = include_str!("fixtures/php/php_real_repo_composer_package.php");
    let result = parse_with(PhpParser::new().unwrap(), source);

    // Types exported
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ContainerInterface".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ContainerAwareTrait".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Container".to_string())
    );

    // Public methods
    assert!(result.metadata.export_names().contains(&"get".to_string()));
    assert!(result.metadata.export_names().contains(&"has".to_string()));
    assert!(result.metadata.export_names().contains(&"add".to_string()));
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"setContainer".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"getContainer".to_string())
    );

    // Private NOT exported
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"resolve".to_string())
    );

    // Trait use inside class
    let fields = result.custom_fields.unwrap();
    let traits_used = fields.get("traits_used").unwrap().as_array().unwrap();
    let tn: Vec<&str> = traits_used.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(tn.contains(&"ContainerAwareTrait"));
}
