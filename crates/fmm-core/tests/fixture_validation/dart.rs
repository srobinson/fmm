use super::*;

#[test]
fn validate_dart_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.dart"
    ));
    let result = parse_fixture(DartParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Public classes
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseWidget".to_string()));
    assert!(names.contains(&"UserProfile".to_string()));
    // Private class excluded
    assert!(!names.contains(&"_InternalHelper".to_string()));

    // Mixins
    assert!(names.contains(&"Loggable".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Enums
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"HttpStatus".to_string()));

    // Extensions
    assert!(names.contains(&"StringExtension".to_string()));
    assert!(names.contains(&"IntExtension".to_string()));

    // Typedefs
    assert!(names.contains(&"Callback".to_string()));
    assert!(names.contains(&"JsonMap".to_string()));
    assert!(!names.contains(&"_PrivateCallback".to_string()));

    // Public functions
    assert!(names.contains(&"globalFunction".to_string()));
    assert!(names.contains(&"processData".to_string()));
    assert!(names.contains(&"asyncOperation".to_string()));
    assert!(!names.contains(&"_privateFunction".to_string()));

    // Top-level variables
    assert!(names.contains(&"appVersion".to_string()));
    assert!(names.contains(&"maxRetries".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));
    assert!(!names.contains(&"_privateVar".to_string()));

    // Imports (package names)
    assert!(result.metadata.imports.contains(&"flutter".to_string()));
    assert!(result.metadata.imports.contains(&"http".to_string()));
    assert!(result.metadata.imports.contains(&"dart:async".to_string()));
    assert!(
        result
            .metadata
            .imports
            .contains(&"dart:convert".to_string())
    );

    // Dependencies (relative paths)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./relative_file.dart".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../utils/helpers.dart".to_string())
    );

    // Custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    assert_eq!(fields.get("mixins").unwrap().as_u64().unwrap(), 2);
    assert_eq!(fields.get("extensions").unwrap().as_u64().unwrap(), 2);

    // LOC
    assert!(result.metadata.loc >= 100);

    assert_exports_sorted(&result);
}
