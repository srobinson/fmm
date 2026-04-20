use crate::support::parse_with;
use fmm_core::parser::builtin::cpp::CppParser;

// C++ validation

/// Modern C++ with templates and smart pointers
#[test]
fn cpp_real_repo_modern_patterns() {
    let source = include_str!("fixtures/cpp/cpp_real_repo_modern_patterns.cpp");
    let result = parse_with(CppParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"EventBus".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Observable".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"EventData".to_string())
    );

    assert!(result.metadata.imports.contains(&"memory".to_string()));
    assert!(result.metadata.imports.contains(&"vector".to_string()));
    assert!(result.metadata.imports.contains(&"functional".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"event.h".to_string())
    );

    let fields = result.custom_fields.unwrap();
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    let ns: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns.contains(&"events"));
}
