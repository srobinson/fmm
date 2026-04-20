use crate::support::parse_with;
use fmm_core::parser::builtin::kotlin::KotlinParser;

// Kotlin validation

/// Android ViewModel pattern — typical MVVM architecture.
#[test]
fn kotlin_real_android_viewmodel_pattern() {
    let source = include_str!("fixtures/kotlin/kotlin_real_android_viewmodel_pattern.kt");
    let result = parse_with(KotlinParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Data class
    assert!(names.contains(&"UiState".to_string()));

    // Sealed class
    assert!(names.contains(&"UiEvent".to_string()));

    // Interface
    assert!(names.contains(&"ViewModelContract".to_string()));

    // Class
    assert!(names.contains(&"MainViewModel".to_string()));

    // Object
    assert!(names.contains(&"ViewModelFactory".to_string()));

    // Private not exported
    assert!(!names.contains(&"loadItems".to_string()));

    // Imports (package roots)
    assert!(
        result
            .metadata
            .imports
            .contains(&"kotlinx.coroutines".to_string())
    );

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("data_classes").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("sealed_classes").unwrap().as_u64().unwrap(), 1);
}

// =============================================================================
// Kotlin validation — Ktor server pattern
// =============================================================================

/// Ktor server pattern — HTTP API setup.
#[test]
fn kotlin_real_ktor_server_pattern() {
    let source = include_str!("fixtures/kotlin/kotlin_real_ktor_server_pattern.kt");
    let result = parse_with(KotlinParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    assert!(names.contains(&"ApiResponse".to_string()));
    assert!(names.contains(&"UserService".to_string()));
    assert!(names.contains(&"UserServiceImpl".to_string()));
    assert!(names.contains(&"configureRouting".to_string()));
    assert!(names.contains(&"API_VERSION".to_string()));
    assert!(names.contains(&"Handler".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"io.ktor".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("data_classes").unwrap().as_u64().unwrap(), 1);
}
