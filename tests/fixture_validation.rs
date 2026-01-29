use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::Parser;

#[test]
fn validate_python_fixture() {
    let source = include_str!("../fixtures/sample.py");
    // Strip FMM header before parsing (parser sees raw source)
    let source_without_header = strip_fmm_header(source, "#");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Expected exports from __all__: fetch_data, transform, DataProcessor, ProcessConfig, MAX_RETRIES
    let expected_exports = vec![
        "DataProcessor",
        "MAX_RETRIES",
        "ProcessConfig",
        "fetch_data",
        "transform",
    ];
    assert_eq!(result.metadata.exports, expected_exports);

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
    let fields = result.custom_fields.expect("should have custom fields");
    let decorators = fields
        .get("decorators")
        .expect("should have decorators")
        .as_array()
        .unwrap();
    let decorator_names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(decorator_names.contains(&"staticmethod"));
    assert!(decorator_names.contains(&"property"));

    // Verify _internal_helper and _INTERNAL_TIMEOUT are NOT exported
    assert!(!result
        .metadata
        .exports
        .contains(&"_internal_helper".to_string()));
    assert!(!result
        .metadata
        .exports
        .contains(&"_INTERNAL_TIMEOUT".to_string()));
}

#[test]
fn validate_rust_fixture() {
    let source = include_str!("../fixtures/sample.rs");
    let source_without_header = strip_fmm_header(source, "//");

    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Expected exports: pub items only (not pub(crate), pub(super), or private)
    let expected_exports = vec!["Config", "Error", "Pipeline", "Status", "process"];
    assert_eq!(result.metadata.exports, expected_exports);

    // Expected imports: anyhow, serde, tokio (external crates, not std)
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));
    assert!(!result.metadata.imports.contains(&"std".to_string()));

    // Expected dependencies: crate, super
    assert!(result.metadata.dependencies.contains(&"crate".to_string()));
    assert!(result.metadata.dependencies.contains(&"super".to_string()));

    // LOC
    assert!(result.metadata.loc > 50);

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");

    // unsafe_blocks: 1
    assert_eq!(
        fields.get("unsafe_blocks").unwrap().as_u64().unwrap(),
        1,
        "should have exactly 1 unsafe block"
    );

    // derives: Clone, Debug, Deserialize, Serialize
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let derive_names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(derive_names.contains(&"Debug"));
    assert!(derive_names.contains(&"Clone"));
    assert!(derive_names.contains(&"Serialize"));
    assert!(derive_names.contains(&"Deserialize"));

    // trait_impls: Display for Error
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let impl_names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(impl_names.contains(&"Display for Error"));

    // lifetimes: 'a, 'static
    let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
    let lt_names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(lt_names.contains(&"'a"));
    assert!(lt_names.contains(&"'static"));

    // async_functions: 1
    assert_eq!(
        fields.get("async_functions").unwrap().as_u64().unwrap(),
        1,
        "should have exactly 1 async function"
    );

    // Verify pub(crate) and pub(super) items are NOT exported
    assert!(!result
        .metadata
        .exports
        .contains(&"internal_helper".to_string()));
    assert!(!result
        .metadata
        .exports
        .contains(&"parent_visible".to_string()));
    assert!(!result.metadata.exports.contains(&"private_fn".to_string()));
}

/// Strip FMM header from fixture file content
fn strip_fmm_header(source: &str, comment_prefix: &str) -> String {
    let start_marker = format!("{} --- FMM ---", comment_prefix);
    let end_marker = format!("{} ---", comment_prefix);

    let mut in_header = false;
    let mut lines: Vec<&str> = Vec::new();

    for line in source.lines() {
        if !in_header && line.trim() == start_marker {
            in_header = true;
            continue;
        }
        if in_header {
            if line.trim() == end_marker {
                in_header = false;
                continue;
            }
            continue;
        }
        lines.push(line);
    }

    lines.join("\n")
}
