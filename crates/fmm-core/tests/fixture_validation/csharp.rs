use super::*;

#[test]
fn validate_csharp_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.cs"
    ));

    let result = parse_fixture(CSharpParser::new().unwrap(), source);

    // Public types with correct declaration line ranges (not namespace line ranges)
    let exports = &result.metadata.exports;
    let find = |name: &str| exports.iter().find(|e| e.name == name).unwrap();

    let ds = find("DataService");
    assert_eq!((ds.start_line, ds.end_line), (8, 29)); // includes [Serializable] attribute

    let transform = find("Transform");
    assert_eq!((transform.start_line, transform.end_line), (18, 22)); // includes [Required] attribute

    let repo = find("IRepository");
    assert_eq!((repo.start_line, repo.end_line), (31, 36));

    let status = find("Status");
    assert_eq!((status.start_line, status.end_line), (38, 44)); // includes [Obsolete] attribute

    let config = find("ProcessConfig");
    assert_eq!((config.start_line, config.end_line), (49, 54));

    // Internal class should NOT be exported
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"InternalHelper".to_string())
    );

    assert_exports_sorted(&result);

    // Using statements
    assert!(result.metadata.imports.contains(&"System".to_string()));

    // Namespaces (custom fields)
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    assert!(!namespaces.is_empty());

    // Attributes
    let attributes = fields.get("attributes").unwrap().as_array().unwrap();
    let attr_names: Vec<&str> = attributes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(attr_names.contains(&"Serializable"));
    assert!(attr_names.contains(&"Obsolete"));
    assert!(attr_names.contains(&"Required"));

    assert!(result.metadata.loc > 40);
}
