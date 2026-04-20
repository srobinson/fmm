use crate::support::parse_with;
use fmm_core::parser::builtin::python::PythonParser;

// Python validation

/// httpx/_content.py — simple module with functions and typed exports
/// Source: https://github.com/encode/httpx (MIT license)
#[test]
fn python_real_repo_httpx_simple_functions() {
    let source = include_str!("fixtures/python/python_real_repo_httpx_simple_functions.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    // Should find both public functions
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"encode_content".to_string()),
        "missing encode_content export"
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"encode_urlencoded_data".to_string()),
        "missing encode_urlencoded_data export"
    );

    // Should find typing import
    assert!(
        result.metadata.imports.contains(&"typing".to_string()),
        "missing typing import"
    );

    assert!(result.metadata.loc > 15, "LOC should be > 15");
}

/// httpx-style __init__.py with __all__ controlling exports
#[test]
fn python_real_repo_httpx_init_with_all() {
    let source = include_str!("fixtures/python/python_real_repo_httpx_init_with_all.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    // __all__ has 21 unique string entries
    assert_eq!(
        result.metadata.export_names().len(),
        21,
        "expected 21 exports from __all__"
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"AsyncClient".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Client".to_string())
    );
    assert!(result.metadata.export_names().contains(&"get".to_string()));
    assert!(result.metadata.export_names().contains(&"post".to_string()));
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"codes".to_string())
    );

    // Relative imports should be in dependencies
    assert!(
        !result.metadata.dependencies.is_empty(),
        "should have relative import deps"
    );
}

/// httpx-style class with decorators and properties
#[test]
fn python_real_repo_httpx_class_with_decorators() {
    let source = include_str!("fixtures/python/python_real_repo_httpx_class_with_decorators.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"QueryParams".to_string())
    );
    assert!(result.metadata.imports.contains(&"typing".to_string()));

    // Should detect decorators
    let fields = result.custom_fields.expect("should have custom fields");
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"property"), "missing property decorator");
    assert!(
        names.contains(&"staticmethod"),
        "missing staticmethod decorator"
    );
}

/// Python module with aliased imports (pandas-style)
#[test]
fn python_real_repo_aliased_and_star_imports() {
    let source = include_str!("fixtures/python/python_real_repo_aliased_and_star_imports.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    // Aliased imports should use the original module name
    assert!(result.metadata.imports.contains(&"numpy".to_string()));
    assert!(result.metadata.imports.contains(&"pandas".to_string()));
    assert!(result.metadata.imports.contains(&"collections".to_string()));
    assert!(result.metadata.imports.contains(&"typing".to_string()));

    // Exports: public class, function, UPPER_CASE constant
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"DataHandler".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"process".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"API_VERSION".to_string())
    );
}

/// FastAPI/Pydantic pattern — decorated classes and functions as primary API surface
/// Source: inspired by real FastAPI applications
#[test]
fn python_real_repo_fastapi_decorated_exports() {
    let source = include_str!("fixtures/python/python_real_repo_fastapi_decorated_exports.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    let names = result.metadata.export_names();
    // Decorated classes
    assert!(
        names.contains(&"AppConfig".to_string()),
        "decorated @dataclass class missing"
    );
    assert!(
        names.contains(&"CacheKey".to_string()),
        "decorated @dataclass(frozen=True) class missing"
    );
    // Bare class
    assert!(
        names.contains(&"RequestBody".to_string()),
        "bare class missing"
    );
    // Decorated functions
    assert!(
        names.contains(&"health_check".to_string()),
        "decorated @app.get function missing"
    );
    assert!(
        names.contains(&"process_item".to_string()),
        "decorated @app.post function missing"
    );
    // Underscore-prefix is Python social convention, not a structural property.
    // fmm surfaces all top-level defs so re-export dereferencing works across
    // modules (e.g. `_port_in_use` re-exported from a barrel `__init__.py`).
    assert!(
        names.contains(&"_internal_validator".to_string()),
        "underscore-prefix top-level defs must be surfaced as exports"
    );

    // Line ranges: AppConfig should start at @dataclass, not class keyword
    let config = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "AppConfig")
        .unwrap();
    assert_eq!(
        config.start_line, 8,
        "AppConfig range should start at @dataclass"
    );
    assert_eq!(config.end_line, 12);

    // CacheKey: @dataclass(frozen=True) decorator
    let cache_key = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "CacheKey")
        .unwrap();
    assert_eq!(
        cache_key.start_line, 18,
        "CacheKey range should start at @dataclass(frozen=True)"
    );

    // Imports
    assert!(result.metadata.imports.contains(&"dataclasses".to_string()));
    assert!(result.metadata.imports.contains(&"pydantic".to_string()));
    assert!(result.metadata.imports.contains(&"fastapi".to_string()));
}

/// Pydantic settings + SQLAlchemy pattern with __all__ controlling decorated exports
#[test]
fn python_real_repo_dunder_all_with_decorated_models() {
    let source =
        include_str!("fixtures/python/python_real_repo_dunder_all_with_decorated_models.py");
    let result = parse_with(PythonParser::new().unwrap(), source);

    // Only __all__ items exported
    let names = result.metadata.export_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"Settings".to_string()));
    assert!(names.contains(&"DatabaseConfig".to_string()));
    assert!(names.contains(&"create_engine".to_string()));

    // DatabaseConfig resolves to decorated definition site
    let db_config = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "DatabaseConfig")
        .unwrap();
    assert_eq!(
        db_config.start_line, 7,
        "DatabaseConfig should resolve to @dataclass line"
    );
    assert_eq!(db_config.end_line, 11);

    // Unlisted items not exported
    assert!(!names.contains(&"_MigrationState".to_string()));
    assert!(!names.contains(&"_run_migrations".to_string()));
}
