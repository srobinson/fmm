use crate::support::parse_with;
use fmm_core::parser::builtin::swift::SwiftParser;

// Swift validation

/// iOS app pattern — typical UIKit-based view controller with networking.
#[test]
fn swift_real_ios_app_pattern() {
    let source = include_str!("fixtures/swift/swift_real_ios_app_pattern.swift");
    let result = parse_with(SwiftParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Protocol
    assert!(names.contains(&"DataSourceDelegate".to_string()));

    // Open class
    assert!(names.contains(&"TableViewController".to_string()));

    // Struct
    assert!(names.contains(&"CellModel".to_string()));

    // Enum
    assert!(names.contains(&"Section".to_string()));

    // Private methods NOT exported
    assert!(!names.contains(&"setupConstraints".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"UIKit".to_string()));
    assert!(result.metadata.imports.contains(&"Foundation".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("protocols").unwrap().as_u64().unwrap(), 1);
}

// =============================================================================
// Swift validation — SwiftUI pattern
// =============================================================================

/// SwiftUI pattern — protocol-oriented with extensions.
#[test]
fn swift_real_swiftui_pattern() {
    let source = include_str!("fixtures/swift/swift_real_swiftui_pattern.swift");
    let result = parse_with(SwiftParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Protocol
    assert!(names.contains(&"Theme".to_string()));

    // Struct
    assert!(names.contains(&"AppTheme".to_string()));

    // Extension method
    assert!(names.contains(&"themed".to_string()));

    // Typealias
    assert!(names.contains(&"ViewBuilder".to_string()));

    // Public let
    assert!(names.contains(&"defaultTheme".to_string()));

    // Import
    assert!(result.metadata.imports.contains(&"SwiftUI".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("protocols").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("extensions").unwrap().as_u64().unwrap(), 1);
}
