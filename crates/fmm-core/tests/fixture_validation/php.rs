use super::*;

#[test]
fn validate_php_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.php"
    ));

    let result = parse_fixture(PhpParser::new().unwrap(), source);

    // Top-level types: classes, interfaces, traits, enums
    let names = result.metadata.export_names();
    assert!(names.contains(&"UserController".to_string()));
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));
    assert!(names.contains(&"Loggable".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"ProcessConfig".to_string()));

    // Top-level functions
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"processQueue".to_string()));

    // Top-level constants
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"API_VERSION".to_string()));

    // Public methods exported
    assert!(names.contains(&"index".to_string()));
    assert!(names.contains(&"show".to_string()));
    assert!(names.contains(&"store".to_string()));
    assert!(names.contains(&"create".to_string()));
    assert!(names.contains(&"isValid".to_string()));

    // Interface public methods
    assert!(names.contains(&"find".to_string()));
    assert!(names.contains(&"save".to_string()));
    assert!(names.contains(&"delete".to_string()));

    // Trait public methods
    assert!(names.contains(&"cacheKey".to_string()));
    assert!(names.contains(&"clearCache".to_string()));
    assert!(names.contains(&"logAction".to_string()));

    // Private/protected methods NOT exported
    assert!(!names.contains(&"validateInput".to_string()));
    assert!(!names.contains(&"authorize".to_string()));

    // Namespace imports
    assert!(result.metadata.imports.contains(&"App".to_string()));
    assert!(result.metadata.imports.contains(&"Illuminate".to_string()));

    // Dependencies (require/include paths)
    assert!(!result.metadata.dependencies.is_empty());

    // Custom fields: namespaces
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let namespaces = fields
        .get("namespaces")
        .expect("should have namespaces")
        .as_array()
        .unwrap();
    assert!(!namespaces.is_empty());

    // Custom fields: traits_used
    let traits_used = fields
        .get("traits_used")
        .expect("should have traits_used")
        .as_array()
        .unwrap();
    let trait_names: Vec<&str> = traits_used.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(trait_names.contains(&"Cacheable"));
    assert!(trait_names.contains(&"Loggable"));

    assert!(result.metadata.loc > 40);
}
