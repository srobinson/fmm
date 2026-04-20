use crate::support::parse_with;
use fmm_core::parser::builtin::c::CParser;

// C validation

/// Linux kernel-style module with init/exit functions, static helpers, and macros.
/// Pattern: public API functions + static internals + struct definitions.
#[test]
fn c_real_repo_linux_kernel_style_module() {
    let source = include_str!("fixtures/c/c_real_repo_linux_kernel_style_module.c");
    let result = parse_with(CParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    // Public functions exported
    assert!(names.contains(&"device_init".to_string()));
    assert!(names.contains(&"device_start".to_string()));
    assert!(names.contains(&"device_cleanup".to_string()));

    // Static function NOT exported
    assert!(!names.contains(&"log_debug".to_string()));

    // Types exported
    assert!(names.contains(&"device_info".to_string()));
    assert!(names.contains(&"device_state".to_string()));

    // Macros
    assert!(names.contains(&"MODULE_NAME".to_string()));
    assert!(names.contains(&"MODULE_VERSION".to_string()));

    // System includes
    assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
    assert!(result.metadata.imports.contains(&"stdlib.h".to_string()));

    // Local dependency
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"module.h".to_string())
    );
}

// =============================================================================
// C validation — embedded systems pattern
// =============================================================================

/// Embedded systems style with hardware register typedefs and ISR patterns.
#[test]
fn c_real_repo_embedded_systems_pattern() {
    let source = include_str!("fixtures/c/c_real_repo_embedded_systems_pattern.c");
    let result = parse_with(CParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    // Public functions
    assert!(names.contains(&"gpio_init".to_string()));
    assert!(names.contains(&"gpio_read_pin".to_string()));
    assert!(names.contains(&"gpio_set_handler".to_string()));

    // Static NOT exported
    assert!(!names.contains(&"default_handler".to_string()));

    // Typedefs
    assert!(names.contains(&"reg32_t".to_string()));
    assert!(names.contains(&"isr_handler_t".to_string()));

    // Struct
    assert!(names.contains(&"gpio_config".to_string()));

    // Macros
    assert!(names.contains(&"GPIO_BASE_ADDR".to_string()));
    assert!(names.contains(&"GPIO_PIN_MASK".to_string()));
    assert!(names.contains(&"MAX_PINS".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    let macros = fields.get("macros").unwrap().as_array().unwrap();
    assert_eq!(macros.len(), 3);
    let typedefs = fields.get("typedefs").unwrap().as_array().unwrap();
    assert_eq!(typedefs.len(), 2);
}
