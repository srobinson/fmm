use super::*;

#[test]
fn validate_elixir_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.ex"
    ));
    let result = parse_fixture(ElixirParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Modules
    assert!(names.contains(&"MyApp.Router".to_string()));
    assert!(names.contains(&"MyApp.Helpers".to_string()));
    assert!(names.contains(&"MyApp.Config".to_string()));

    // Public functions
    assert!(names.contains(&"handle".to_string()));
    assert!(names.contains(&"helper_function".to_string()));
    assert!(names.contains(&"another_helper".to_string()));
    assert!(names.contains(&"get".to_string()));
    assert!(names.contains(&"set".to_string()));

    // Private functions excluded
    assert!(!names.contains(&"private_handler".to_string()));
    assert!(!names.contains(&"internal_work".to_string()));

    // Public macros
    assert!(names.contains(&"route".to_string()));
    assert!(!names.contains(&"private_macro".to_string()));

    // Public guards
    assert!(names.contains(&"is_valid".to_string()));
    assert!(!names.contains(&"is_internal".to_string()));

    // Delegates
    assert!(names.contains(&"format".to_string()));

    // Protocols
    assert!(names.contains(&"Printable".to_string()));
    assert!(names.contains(&"print".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"Plug".to_string()));
    assert!(result.metadata.imports.contains(&"Logger".to_string()));
    assert!(result.metadata.imports.contains(&"MyApp".to_string()));
    assert!(result.metadata.imports.contains(&"EEx".to_string()));
    assert!(result.metadata.imports.contains(&"GenServer".to_string()));
    assert!(result.metadata.imports.contains(&"Enum".to_string()));

    // Custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    assert_eq!(fields.get("macros").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("protocols").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("behaviours").unwrap().as_u64().unwrap(), 1);

    // LOC
    assert!(result.metadata.loc >= 73);

    assert_exports_sorted(&result);
}

// --- TypeScript fixtures (ALP-757) --------------------------------------------
