use super::*;

#[test]
fn validate_typescript_sample_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.ts"
    ));
    let result = parse_fixture(TypeScriptParser::new().unwrap(), source);

    // All major TypeScript export forms are present
    let names = result.metadata.export_names();
    assert!(names.contains(&"AppConfig".to_string()), "interface");
    assert!(names.contains(&"Handler".to_string()), "type alias");
    assert!(names.contains(&"Status".to_string()), "enum");
    assert!(names.contains(&"DEFAULT_PORT".to_string()), "const");
    assert!(names.contains(&"AppService".to_string()), "class");
    assert!(names.contains(&"createApp".to_string()), "function");

    // External packages land in imports
    assert!(result.metadata.imports.contains(&"events".to_string()));
    assert!(result.metadata.imports.contains(&"fs/promises".to_string()));

    // Relative imports land in dependencies
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./helper".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../config".to_string())
    );

    assert!(result.metadata.loc > 30);

    assert_exports_sorted(&result);
}

#[test]
fn validate_typescript_barrel_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/typescript/barrel.ts"
    ));
    let result = parse_fixture(TypeScriptParser::new().unwrap(), source);

    // Named re-exports are indexed
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"UserService".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"AuthService".to_string())
    );

    // export * as models => namespace name indexed (ALP-755)
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"models".to_string())
    );

    // export * from './utils' => no export name (ALP-750)
    assert!(!result.metadata.export_names().contains(&"*".to_string()));

    // All re-export sources captured as dependencies (ALP-749/750)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./user.service".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./auth.service".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./utils".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./models".to_string())
    );
}

#[test]
fn validate_typescript_decorators_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/typescript/decorators.ts"
    ));
    let result = parse_fixture(TypeScriptParser::new().unwrap(), source);

    // Exported classes are indexed
    let names = result.metadata.export_names();
    assert!(names.contains(&"UserService".to_string()));
    assert!(names.contains(&"UserController".to_string()));
    assert!(names.contains(&"AppModule".to_string()));
    assert!(names.contains(&"PlainService".to_string()));

    // Decorators are extracted into custom_fields (ALP-754)
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom_fields");
    let decorators: Vec<&str> = fields["decorators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(decorators.contains(&"Injectable"), "call-form decorator");
    assert!(decorators.contains(&"Controller"), "call-form with arg");
    assert!(decorators.contains(&"Module"), "call-form with object arg");

    // No external imports or dependencies
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
}
