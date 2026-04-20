use super::*;

#[test]
fn validate_cpp_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.cpp"
    ));

    let result = parse_fixture(CppParser::new().unwrap(), source);

    // Classes, structs, enums, functions, templates with correct declaration line ranges
    let exports = &result.metadata.exports;
    let find = |name: &str| exports.iter().find(|e| e.name == name).unwrap();

    let point = find("Point");
    assert_eq!((point.start_line, point.end_line), (10, 12));

    let status = find("Status");
    assert_eq!((status.start_line, status.end_line), (14, 18));

    let config = find("Config");
    assert_eq!((config.start_line, config.end_line), (20, 28));

    let engine = find("Engine");
    assert_eq!((engine.start_line, engine.end_line), (30, 39));

    let pipeline = find("Pipeline");
    assert_eq!((pipeline.start_line, pipeline.end_line), (41, 54)); // template_declaration

    let process = find("process");
    assert_eq!((process.start_line, process.end_line), (60, 64));

    // System includes
    assert!(result.metadata.imports.contains(&"vector".to_string()));
    assert!(result.metadata.imports.contains(&"string".to_string()));
    assert!(result.metadata.imports.contains(&"memory".to_string()));
    assert!(result.metadata.imports.contains(&"algorithm".to_string()));

    // Local includes (dependencies)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"config.h".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"utils/helpers.h".to_string())
    );

    // Namespaces (custom fields)
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let namespaces = fields
        .get("namespaces")
        .expect("should have namespaces")
        .as_array()
        .unwrap();
    let ns_names: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns_names.contains(&"engine"));
    assert!(ns_names.contains(&"utils"));

    assert_exports_sorted(&result);

    assert!(result.metadata.loc > 50);
}
