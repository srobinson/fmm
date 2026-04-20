use crate::support::parse_with;
use fmm_core::parser::builtin::typescript::TypeScriptParser;

// TypeScript validation

/// TypeScript barrel file with re-exports
#[test]
fn typescript_real_repo_barrel_file() {
    let source = include_str!("fixtures/typescript/typescript_real_repo_barrel_file.ts");
    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert_eq!(result.metadata.export_names().len(), 4);
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"createContext".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"parseMarkdown".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"renderOutput".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"validateConfig".to_string())
    );

    // ALP-749: re-export sources must appear in dependencies
    assert!(result.metadata.imports.is_empty());
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./context".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./parser".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./renderer".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./config".to_string())
    );
}

/// TypeScript with interfaces, classes, and async methods
#[test]
fn typescript_real_repo_complex_module() {
    let source = include_str!("fixtures/typescript/typescript_real_repo_complex_module.ts");
    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ConnectionOptions".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"ConnectionManager".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"createConnection".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"DEFAULT_PORT".to_string())
    );

    // External imports
    assert!(result.metadata.imports.contains(&"events".to_string()));
    assert!(result.metadata.imports.contains(&"winston".to_string()));

    // Relative dependencies
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./config".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"../database".to_string())
    );
}

/// TypeScript internal module (no exports)
#[test]
fn typescript_real_repo_internal_module() {
    let source = include_str!("fixtures/typescript/typescript_real_repo_internal_module.ts");
    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    // ALP-922: nested extraction may add closure-state from non-exported functions;
    // check that there are no top-level (non-nested) exports.
    assert!(
        result.metadata.export_names().is_empty(),
        "internal module should have no top-level exports, got: {:?}",
        result.metadata.export_names()
    );
    assert!(result.metadata.imports.contains(&"fs".to_string()));
    assert!(result.metadata.imports.contains(&"path".to_string()));
}

/// TypeScript with default exports, type aliases, and named exports mixed
#[test]
fn typescript_real_repo_default_and_type_exports() {
    let source =
        include_str!("fixtures/typescript/typescript_real_repo_default_and_type_exports.ts");
    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert_eq!(
        result.metadata.export_names(),
        vec!["AppProps", "AppState", "APP_VERSION", "App"]
    );
    assert!(result.metadata.imports.contains(&"react".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./store".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./theme".to_string())
    );
}

/// TypeScript default export of existing binding (common React pattern)
#[test]
fn typescript_real_repo_default_export_identifier() {
    let source =
        include_str!("fixtures/typescript/typescript_real_repo_default_export_identifier.ts");
    let result = parse_with(TypeScriptParser::new().unwrap(), source);

    assert_eq!(result.metadata.export_names(), vec!["ConnectedProfile"]);
    assert!(result.metadata.imports.contains(&"react-redux".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./actions".to_string())
    );
}
