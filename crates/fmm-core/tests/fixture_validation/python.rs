use super::*;

#[test]
fn validate_python_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.py"
    ));

    let result = parse_fixture(PythonParser::new().unwrap(), source);

    // Expected exports from __all__, sorted by definition site line number
    let expected_exports = vec![
        "MAX_RETRIES",
        "ProcessConfig",
        "DataProcessor",
        "fetch_data",
        "transform",
    ];
    assert_eq!(result.metadata.export_names(), expected_exports);

    // Expected imports: requests, pandas (external packages)
    // pathlib is stdlib but still extracted as import
    assert!(result.metadata.imports.contains(&"requests".to_string()));
    assert!(result.metadata.imports.contains(&"pandas".to_string()));
    assert!(result.metadata.imports.contains(&"pathlib".to_string()));

    // Expected dependencies: .utils, ..models (relative imports)
    assert!(!result.metadata.dependencies.is_empty());

    // LOC should match fixture
    assert!(result.metadata.loc > 40);

    // Custom fields: decorators
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let decorators = fields
        .get("decorators")
        .expect("should have decorators")
        .as_array()
        .unwrap();
    let decorator_names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(decorator_names.contains(&"staticmethod"));
    assert!(decorator_names.contains(&"property"));

    // Verify _internal_helper and _INTERNAL_TIMEOUT are NOT exported
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"_internal_helper".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"_INTERNAL_TIMEOUT".to_string())
    );
}

#[test]
fn validate_python_heuristic_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/python/heuristic.py"
    ));

    let result = parse_fixture(PythonParser::new().unwrap(), source);

    // Path B (no __all__): decorated and bare classes/functions exported
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Agent".to_string()),
        "decorated class missing"
    );
    assert!(names.contains(&"Router".to_string()), "bare class missing");
    assert!(
        names.contains(&"handle_request".to_string()),
        "bare function missing"
    );
    assert!(
        names.contains(&"cached_lookup".to_string()),
        "decorated function missing"
    );
    assert!(
        names.contains(&"MAX_CONNECTIONS".to_string()),
        "constant missing"
    );

    // Underscore-prefix top-level defs are surfaced so barrel re-export
    // dereferencing (e.g. `_port_in_use` re-exported from __init__.py)
    // can resolve to the origin def line.
    assert!(names.contains(&"_internal_setup".to_string()));
    assert!(names.contains(&"_Registry".to_string()));

    // Line range for decorated class should start at decorator
    let agent = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "Agent")
        .unwrap();
    assert_eq!(
        agent.start_line, 7,
        "Agent range should start at @dataclass"
    );
}

#[test]
fn validate_python_decorated_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/python/decorated.py"
    ));

    let result = parse_fixture(PythonParser::new().unwrap(), source);

    let names = result.metadata.export_names();
    assert!(names.contains(&"SimpleDecorated".to_string()));
    assert!(names.contains(&"DecoratedWithArgs".to_string()));
    assert!(names.contains(&"multi_decorated".to_string()));
    assert!(names.contains(&"bare_function".to_string()));
    assert!(names.contains(&"BareClass".to_string()));

    // Underscore-prefix decorated classes are still surfaced: no structural filter.
    assert!(names.contains(&"_PrivateDecorated".to_string()));

    // Decorators captured in custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let deco_names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(deco_names.contains(&"dataclass"));
    assert!(deco_names.contains(&"lru_cache"));
    assert!(deco_names.contains(&"staticmethod"));
}

#[test]
fn validate_python_with_all_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/python/with_all.py"
    ));

    let result = parse_fixture(PythonParser::new().unwrap(), source);

    // Path A: only __all__ names exported
    let names = result.metadata.export_names();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"DecoratedModel".to_string()));
    assert!(names.contains(&"process".to_string()));
    assert!(names.contains(&"APP_NAME".to_string()));

    // Not in __all__
    assert!(!names.contains(&"_InternalModel".to_string()));
    assert!(!names.contains(&"unlisted_helper".to_string()));

    // DecoratedModel resolves to definition site, not __all__ line
    let model = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "DecoratedModel")
        .unwrap();
    assert!(
        model.start_line > 6,
        "DecoratedModel should resolve to @dataclass line, not __all__ line"
    );
}
