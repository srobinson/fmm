use super::*;

#[test]
fn validate_scala_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.scala"
    ));

    let result = parse_fixture(ScalaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Case classes
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"Success".to_string()));
    assert!(names.contains(&"Failure".to_string()));

    // Traits
    assert!(names.contains(&"Processor".to_string()));
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Result".to_string()));

    // Classes
    assert!(names.contains(&"DataService".to_string()));
    assert!(names.contains(&"LegacyProcessor".to_string()));

    // Objects
    assert!(names.contains(&"DataService".to_string()));
    assert!(names.contains(&"Pipeline".to_string()));

    // Top-level function
    assert!(names.contains(&"transform".to_string()));

    // Top-level val/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"globalState".to_string()));

    // Implicit def
    assert!(names.contains(&"stringToConfig".to_string()));

    // Private items should NOT be exported
    assert!(!names.contains(&"InternalHelper".to_string()));
    assert!(!names.contains(&"InternalUtils".to_string()));

    // Imports (root packages)
    assert!(result.metadata.imports.contains(&"scala".to_string()));
    assert!(result.metadata.imports.contains(&"akka".to_string()));
    assert!(result.metadata.imports.contains(&"com".to_string()));

    // Custom fields: case_classes
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let cc = fields
        .get("case_classes")
        .expect("should have case_classes")
        .as_array()
        .unwrap();
    let cc_names: Vec<&str> = cc.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(cc_names.contains(&"Config"));
    assert!(cc_names.contains(&"Status"));
    assert!(cc_names.contains(&"Success"));
    assert!(cc_names.contains(&"Failure"));

    // Custom fields: implicits
    assert_eq!(
        fields.get("implicits").unwrap().as_u64().unwrap(),
        1,
        "should have 1 implicit definition"
    );

    // Custom fields: annotations
    let annotations = fields
        .get("annotations")
        .expect("should have annotations")
        .as_array()
        .unwrap();
    let ann_names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ann_names.contains(&"deprecated"));
    assert!(ann_names.contains(&"volatile"));

    // LOC
    assert!(result.metadata.loc > 40);

    assert_exports_sorted(&result);
}

// =============================================================================
// Swift fixture validation
// =============================================================================
