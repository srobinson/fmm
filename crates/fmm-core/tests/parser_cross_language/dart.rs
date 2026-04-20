use crate::support::parse_with;
use fmm_core::parser::builtin::dart::DartParser;

// Dart validation

#[test]
fn dart_real_flutter_widget_pattern() {
    // Pattern: Flutter widget with state management
    let source = include_str!("fixtures/dart/dart_real_flutter_widget_pattern.dart");
    let result = parse_with(DartParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    // Public widgets
    assert!(names.contains(&"CounterApp".to_string()));
    assert!(names.contains(&"CounterPage".to_string()));
    assert!(names.contains(&"CounterModel".to_string()));
    assert!(names.contains(&"AppRoute".to_string()));
    assert!(names.contains(&"WidgetCallback".to_string()));

    // Private state class excluded
    assert!(!names.contains(&"_CounterPageState".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"flutter".to_string()));
    assert!(result.metadata.imports.contains(&"provider".to_string()));
}

#[test]
fn dart_real_data_layer_pattern() {
    // Pattern: Repository / service layer
    let source = include_str!("fixtures/dart/dart_real_data_layer_pattern.dart");
    let result = parse_with(DartParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"UserRepository".to_string()));
    assert!(names.contains(&"ApiClient".to_string()));
    assert!(names.contains(&"JsonMap".to_string()));
    assert!(!names.contains(&"_initializeClient".to_string()));

    assert!(
        result
            .metadata
            .imports
            .contains(&"dart:convert".to_string())
    );
    assert!(result.metadata.imports.contains(&"http".to_string()));
}
