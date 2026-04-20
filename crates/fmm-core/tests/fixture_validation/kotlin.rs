use super::*;

#[test]
fn validate_kotlin_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.kt"
    ));

    let result = parse_fixture(KotlinParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Classes (default public)
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseRepository".to_string()));
    assert!(names.contains(&"ServiceLocator".to_string()));

    // Data classes
    assert!(names.contains(&"UserProfile".to_string()));
    assert!(names.contains(&"APIResponse".to_string()));

    // Sealed class
    assert!(names.contains(&"Result".to_string()));

    // Interfaces
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Objects
    assert!(names.contains(&"AppConfig".to_string()));
    assert!(names.contains(&"DatabaseManager".to_string()));

    // Enum classes
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"HttpStatus".to_string()));

    // Top-level functions
    assert!(names.contains(&"createManager".to_string()));
    assert!(names.contains(&"processData".to_string()));
    assert!(names.contains(&"oldMethod".to_string()));
    assert!(names.contains(&"asyncOperation".to_string()));

    // Top-level val/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));
    assert!(names.contains(&"VERSION".to_string()));

    // Typealias
    assert!(names.contains(&"StringMap".to_string()));
    assert!(names.contains(&"Callback".to_string()));

    // Private/internal should NOT be exported
    assert!(!names.contains(&"InternalHelper".to_string()));
    assert!(!names.contains(&"ModuleInternal".to_string()));
    assert!(!names.contains(&"hiddenFunction".to_string()));
    assert!(!names.contains(&"moduleFunction".to_string()));

    // Imports (package roots: first two segments)
    assert!(
        result
            .metadata
            .imports
            .contains(&"kotlin.collections".to_string())
    );
    assert!(result.metadata.imports.contains(&"java.util".to_string()));
    assert!(result.metadata.imports.contains(&"org.example".to_string()));

    // Dependencies (empty for Kotlin)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    assert_eq!(
        fields.get("data_classes").unwrap().as_u64().unwrap(),
        2,
        "should have 2 data class declarations"
    );
    assert_eq!(
        fields.get("sealed_classes").unwrap().as_u64().unwrap(),
        1,
        "should have 1 sealed class declaration"
    );
    assert_eq!(
        fields.get("companion_objects").unwrap().as_u64().unwrap(),
        1,
        "should have 1 companion object"
    );

    // LOC
    assert!(result.metadata.loc > 80);

    assert_exports_sorted(&result);
}
