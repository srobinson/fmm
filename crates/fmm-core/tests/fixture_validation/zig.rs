use super::*;

#[test]
fn validate_zig_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.zig"
    ));

    let result = parse_fixture(ZigParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Pub const values
    assert!(names.contains(&"MAX_RETRIES".to_string()));

    // Pub var
    assert!(names.contains(&"debug_enabled".to_string()));

    // Pub const types (struct, enum, error, union)
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"PipelineError".to_string()));
    assert!(names.contains(&"ArrayList".to_string()));
    assert!(names.contains(&"Value".to_string()));

    // Pub functions
    assert!(names.contains(&"processBatch".to_string()));
    assert!(names.contains(&"transform".to_string()));

    // Non-pub items should NOT be exported
    assert!(!names.contains(&"internal_timeout".to_string()));
    assert!(!names.contains(&"internalHelper".to_string()));
    assert!(!names.contains(&"validateInput".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"builtin".to_string()));

    // Dependencies (relative imports)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./utils.zig".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../config.zig".to_string())
    );

    // Custom fields: comptime_blocks and test_blocks
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    assert_eq!(
        fields.get("comptime_blocks").unwrap().as_u64().unwrap(),
        2,
        "should have 2 comptime blocks"
    );
    assert_eq!(
        fields.get("test_blocks").unwrap().as_u64().unwrap(),
        3,
        "should have 3 test blocks"
    );

    // LOC
    assert!(result.metadata.loc > 40);

    assert_exports_sorted(&result);
}
