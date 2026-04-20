use super::*;

#[test]
fn validate_lua_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.lua"
    ));

    let result = parse_fixture(LuaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Module methods (M.name pattern)
    assert!(names.contains(&"init".to_string()));
    assert!(names.contains(&"process".to_string()));
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"status".to_string()));
    assert!(names.contains(&"reset".to_string()));

    // Global functions
    assert!(names.contains(&"create_connection".to_string()));
    assert!(names.contains(&"parse_config".to_string()));

    // Local functions should NOT be exported
    assert!(!names.contains(&"validate_input".to_string()));
    assert!(!names.contains(&"format_output".to_string()));
    assert!(!names.contains(&"log_action".to_string()));

    // Imports (require calls with non-relative paths)
    assert!(result.metadata.imports.contains(&"cjson".to_string()));
    assert!(result.metadata.imports.contains(&"socket".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));

    // Dependencies (require calls with relative paths)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./config".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../lib/utils".to_string())
    );

    // No custom fields for Lua
    assert!(result.custom_fields.is_none());

    // LOC
    assert!(result.metadata.loc > 40);

    assert_exports_sorted(&result);
}
