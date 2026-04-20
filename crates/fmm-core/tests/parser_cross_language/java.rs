use crate::support::parse_with;
use fmm_core::parser::builtin::java::JavaParser;

// Java validation

/// Spring Boot controller with annotations
#[test]
fn java_real_repo_spring_controller() {
    let source = include_str!("fixtures/java/java_real_repo_spring_controller.java");
    let result = parse_with(JavaParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"UserController".to_string())
    );
    // ALP-771: public methods are now method entries with parent_class, not in export_names()
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"getUsers".to_string()),
        "getUsers should NOT be in flat exports"
    );
    assert!(
        result
            .metadata
            .exports
            .iter()
            .any(|e| e.parent_class.as_deref() == Some("UserController") && e.name == "getUsers"),
        "UserController.getUsers should be a method entry"
    );
    assert!(
        result
            .metadata
            .exports
            .iter()
            .any(|e| e.parent_class.as_deref() == Some("UserController") && e.name == "createUser"),
        "UserController.createUser should be a method entry"
    );
    assert!(
        !result.metadata.exports.iter().any(|e| e.name == "validate"),
        "private validate() should NOT be indexed at all"
    );

    let fields = result.custom_fields.unwrap();
    let annotations = fields.get("annotations").unwrap().as_array().unwrap();
    let names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"RestController"));
    assert!(names.contains(&"GetMapping"));
    assert!(names.contains(&"PostMapping"));
}

/// Java generics and interface patterns
#[test]
fn java_real_repo_generics_and_interfaces() {
    let source = include_str!("fixtures/java/java_real_repo_generics_and_interfaces.java");
    let result = parse_with(JavaParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Validator".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Priority".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"StringValidator".to_string())
    );
}
