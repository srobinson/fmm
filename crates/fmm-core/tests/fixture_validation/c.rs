use super::*;

#[test]
fn validate_c_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.c"
    ));

    let result = parse_fixture(CParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Macros
    assert!(names.contains(&"MAX_BUFFER_SIZE".to_string()));
    assert!(names.contains(&"MIN".to_string()));
    assert!(names.contains(&"API_VERSION".to_string()));

    // Typedefs
    assert!(names.contains(&"Callback".to_string()));
    assert!(names.contains(&"HashValue".to_string()));

    // Structs and enums
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Result".to_string()));

    // Non-static functions (including pointer-returning ones)
    assert!(names.contains(&"config_init".to_string()));
    assert!(names.contains(&"process_data".to_string()));
    assert!(names.contains(&"config_free".to_string()));
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"get_buffer".to_string()));
    assert!(names.contains(&"compute_hash".to_string()));

    // Static functions should NOT be exported
    assert!(!names.contains(&"validate_input".to_string()));
    assert!(!names.contains(&"log_message".to_string()));

    // System includes => imports
    assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
    assert!(result.metadata.imports.contains(&"stdlib.h".to_string()));
    assert!(result.metadata.imports.contains(&"string.h".to_string()));

    // Local includes => dependencies
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

    // Custom fields: macros
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let macros = fields
        .get("macros")
        .expect("should have macros")
        .as_array()
        .unwrap();
    let macro_names: Vec<&str> = macros.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(macro_names.contains(&"MAX_BUFFER_SIZE"));
    assert!(macro_names.contains(&"MIN"));
    assert!(macro_names.contains(&"API_VERSION"));

    // Custom fields: typedefs
    let typedefs = fields
        .get("typedefs")
        .expect("should have typedefs")
        .as_array()
        .unwrap();
    let typedef_names: Vec<&str> = typedefs.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(typedef_names.contains(&"Callback"));
    assert!(typedef_names.contains(&"HashValue"));

    // LOC
    assert!(result.metadata.loc > 40);

    assert_exports_sorted(&result);
}
