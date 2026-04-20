use crate::support::parse_with;
use fmm_core::parser::builtin::zig::ZigParser;

// Zig validation

/// Zig allocator wrapper pattern — common in Zig libraries for custom memory allocation.
/// Inspired by patterns from std.mem.Allocator usage in the Zig standard library.
#[test]
fn zig_real_allocator_pattern() {
    let source = include_str!("fixtures/zig/zig_real_allocator_pattern.zig");
    let result = parse_with(ZigParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Pub structs
    assert!(names.contains(&"ArenaAllocator".to_string()));
    assert!(names.contains(&"FixedBufferAllocator".to_string()));

    // Pub function
    assert!(names.contains(&"createAllocator".to_string()));

    // Total top-level exports: 3
    assert_eq!(names.len(), 3);

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));

    // Dependencies
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./log.zig".to_string())
    );

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("test_blocks").unwrap().as_u64().unwrap(), 1);
}

// =============================================================================
// Zig validation — build.zig pattern (Zig build system configuration)
// =============================================================================

/// Zig build configuration pattern — typical build.zig structure.
/// Inspired by real Zig project build files.
#[test]
fn zig_real_build_zig_pattern() {
    let source = include_str!("fixtures/zig/zig_real_build_zig_pattern.zig");
    let result = parse_with(ZigParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Pub struct
    assert!(names.contains(&"Package".to_string()));

    // Pub function (the build function)
    assert!(names.contains(&"build".to_string()));

    // Pub const values
    assert!(names.contains(&"version".to_string()));
    assert!(names.contains(&"min_zig_version".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"builtin".to_string()));

    // No dependencies (no relative imports)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields: comptime but no tests
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("comptime_blocks").unwrap().as_u64().unwrap(), 1);
    assert!(!fields.contains_key("test_blocks"));
}
