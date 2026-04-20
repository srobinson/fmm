use super::*;

#[test]
fn validate_java_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.java"
    ));

    let result = parse_fixture(JavaParser::new().unwrap(), source);

    // Top-level classes, interfaces, enums
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
            .contains(&"Repository".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Status".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ProcessConfig".to_string())
    );

    // ALP-771: public methods are now method entries with parent_class, not in export_names()
    assert!(
        result
            .metadata
            .exports
            .iter()
            .any(|e| e.parent_class.as_deref() == Some("DataProcessor") && e.name == "process"),
        "DataProcessor.process should be a method entry"
    );
    assert!(
        result
            .metadata
            .exports
            .iter()
            .any(|e| e.parent_class.as_deref() == Some("DataProcessor") && e.name == "transform"),
        "DataProcessor.transform should be a method entry"
    );
    // Methods must NOT appear in flat export_names()
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"process".to_string()),
        "process should NOT be in flat export_names()"
    );

    // Private methods should not be exported
    assert!(!result.metadata.exports.iter().any(|e| e.name == "validate"));

    // Imports
    assert!(result.metadata.imports.contains(&"java.util".to_string()));
    assert!(
        result
            .metadata
            .imports
            .contains(&"org.springframework".to_string())
    );

    // Annotations (custom fields)
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let annotations = fields
        .get("annotations")
        .expect("should have annotations")
        .as_array()
        .unwrap();
    let ann_names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ann_names.contains(&"Service"));
    assert!(ann_names.contains(&"Override"));
    assert!(ann_names.contains(&"Deprecated"));
    assert!(ann_names.contains(&"FunctionalInterface"));

    assert!(result.metadata.loc > 40);
}
