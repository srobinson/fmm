use super::*;

#[test]
fn validate_swift_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.swift"
    ));

    let result = parse_fixture(SwiftParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Public classes
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseViewController".to_string()));

    // Public structs
    assert!(names.contains(&"Point".to_string()));
    assert!(names.contains(&"APIConfig".to_string()));

    // Public enums
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"NetworkError".to_string()));

    // Protocols
    assert!(names.contains(&"Drawable".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Top-level public function
    assert!(names.contains(&"createManager".to_string()));

    // Public let/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));

    // Public typealias
    assert!(names.contains(&"JSONDictionary".to_string()));
    assert!(names.contains(&"CompletionHandler".to_string()));

    // Public extension methods
    assert!(names.contains(&"trimmed".to_string()));
    assert!(names.contains(&"uniqueElements".to_string()));

    // Internal/private/fileprivate/default should NOT be exported
    assert!(!names.contains(&"InternalConfig".to_string()));
    assert!(!names.contains(&"helperFunction".to_string()));
    assert!(!names.contains(&"secretFunction".to_string()));
    assert!(!names.contains(&"DefaultAccessStruct".to_string()));
    assert!(!names.contains(&"defaultAccessFunc".to_string()));
    assert!(!names.contains(&"internalRetry".to_string()));
    assert!(!names.contains(&"defaultAccessMethod".to_string()));

    // Non-public extension methods should NOT be exported
    assert!(!names.contains(&"doubled".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"Foundation".to_string()));
    assert!(result.metadata.imports.contains(&"UIKit".to_string()));
    assert!(
        result
            .metadata
            .imports
            .contains(&"MyTestModule".to_string())
    );

    // Dependencies (Swift has none)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    assert_eq!(
        fields.get("protocols").unwrap().as_u64().unwrap(),
        2,
        "should have 2 protocol declarations"
    );
    assert_eq!(
        fields.get("extensions").unwrap().as_u64().unwrap(),
        3,
        "should have 3 extension declarations"
    );

    // LOC
    assert!(result.metadata.loc > 100);

    assert_exports_sorted(&result);
}

// =============================================================================
// Kotlin fixture validation
// =============================================================================
