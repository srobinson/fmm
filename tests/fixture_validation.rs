use fmm::parser::builtin::cpp::CppParser;
use fmm::parser::builtin::csharp::CSharpParser;
use fmm::parser::builtin::go::GoParser;
use fmm::parser::builtin::java::JavaParser;
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::ruby::RubyParser;
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

#[test]
fn validate_go_fixture() {
    let source = include_str!("../fixtures/sample.go");
    let source_without_header = strip_fmm_header(source, "//");

    let mut parser = GoParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Exported: capitalized names only (StatusActive, StatusInactive are iota consts)
    let expected_exports = vec![
        "Config",
        "Handler",
        "MaxRetries",
        "NewHandler",
        "Process",
        "Status",
        "StatusActive",
        "StatusInactive",
    ];
    assert_eq!(result.metadata.exports, expected_exports);

    // Imports: stdlib packages
    assert!(result
        .metadata
        .imports
        .contains(&"encoding/json".to_string()));
    assert!(result.metadata.imports.contains(&"fmt".to_string()));
    assert!(result.metadata.imports.contains(&"net/http".to_string()));

    // Dependencies: external modules (contain dots)
    assert!(result
        .metadata
        .dependencies
        .contains(&"github.com/gin-gonic/gin".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"github.com/redis/go-redis/v9".to_string()));

    // Non-exported items should not be in exports
    assert!(!result
        .metadata
        .exports
        .contains(&"internalTimeout".to_string()));
    assert!(!result
        .metadata
        .exports
        .contains(&"privateState".to_string()));
    assert!(!result.metadata.exports.contains(&"helperFunc".to_string()));

    assert!(result.metadata.loc > 50);
}

#[test]
fn validate_java_fixture() {
    let source = include_str!("../fixtures/sample.java");
    let source_without_header = strip_fmm_header(source, "//");

    let mut parser = JavaParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Top-level classes, interfaces, enums
    assert!(result
        .metadata
        .exports
        .contains(&"DataProcessor".to_string()));
    assert!(result.metadata.exports.contains(&"Repository".to_string()));
    assert!(result.metadata.exports.contains(&"Status".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"ProcessConfig".to_string()));

    // Public methods
    assert!(result.metadata.exports.contains(&"process".to_string()));
    assert!(result.metadata.exports.contains(&"transform".to_string()));

    // Private methods should not be exported
    assert!(!result.metadata.exports.contains(&"validate".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"java.util".to_string()));
    assert!(result
        .metadata
        .imports
        .contains(&"org.springframework".to_string()));

    // Annotations (custom fields)
    let fields = result.custom_fields.expect("should have custom fields");
    let annotations = fields
        .get("annotations")
        .expect("should have annotations")
        .as_array()
        .unwrap();
    let ann_names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ann_names.contains(&"Service"));
    assert!(ann_names.contains(&"Override"));
    assert!(ann_names.contains(&"Deprecated"));
    assert!(ann_names.contains(&"FunctionalInterface"));

    assert!(result.metadata.loc > 40);
}

#[test]
fn validate_cpp_fixture() {
    let source = include_str!("../fixtures/sample.cpp");
    let source_without_header = strip_fmm_header(source, "//");

    let mut parser = CppParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Classes, structs, enums, functions, templates
    assert!(result.metadata.exports.contains(&"Engine".to_string()));
    assert!(result.metadata.exports.contains(&"Config".to_string()));
    assert!(result.metadata.exports.contains(&"Point".to_string()));
    assert!(result.metadata.exports.contains(&"Status".to_string()));
    assert!(result.metadata.exports.contains(&"Pipeline".to_string()));
    assert!(result.metadata.exports.contains(&"process".to_string()));

    // System includes
    assert!(result.metadata.imports.contains(&"vector".to_string()));
    assert!(result.metadata.imports.contains(&"string".to_string()));
    assert!(result.metadata.imports.contains(&"memory".to_string()));
    assert!(result.metadata.imports.contains(&"algorithm".to_string()));

    // Local includes (dependencies)
    assert!(result
        .metadata
        .dependencies
        .contains(&"config.h".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"utils/helpers.h".to_string()));

    // Namespaces (custom fields)
    let fields = result.custom_fields.expect("should have custom fields");
    let namespaces = fields
        .get("namespaces")
        .expect("should have namespaces")
        .as_array()
        .unwrap();
    let ns_names: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns_names.contains(&"engine"));
    assert!(ns_names.contains(&"utils"));

    assert!(result.metadata.loc > 50);
}

#[test]
fn validate_csharp_fixture() {
    let source = include_str!("../fixtures/sample.cs");
    let source_without_header = strip_fmm_header(source, "//");

    let mut parser = CSharpParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Public types only
    assert!(result.metadata.exports.contains(&"DataService".to_string()));
    assert!(result.metadata.exports.contains(&"IRepository".to_string()));
    assert!(result.metadata.exports.contains(&"Status".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"ProcessConfig".to_string()));
    assert!(result.metadata.exports.contains(&"Transform".to_string()));

    // Internal class should NOT be exported
    assert!(!result
        .metadata
        .exports
        .contains(&"InternalHelper".to_string()));

    // Using statements
    assert!(result.metadata.imports.contains(&"System".to_string()));

    // Namespaces (custom fields)
    let fields = result.custom_fields.expect("should have custom fields");
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    assert!(!namespaces.is_empty());

    // Attributes
    let attributes = fields.get("attributes").unwrap().as_array().unwrap();
    let attr_names: Vec<&str> = attributes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(attr_names.contains(&"Serializable"));
    assert!(attr_names.contains(&"Obsolete"));
    assert!(attr_names.contains(&"Required"));

    assert!(result.metadata.loc > 40);
}

#[test]
fn validate_ruby_fixture() {
    let source = include_str!("../fixtures/sample.rb");
    let source_without_header = strip_fmm_header(source, "#");

    let mut parser = RubyParser::new().unwrap();
    let result = parser.parse(&source_without_header).unwrap();

    // Classes, modules, top-level methods
    assert!(result
        .metadata
        .exports
        .contains(&"DataProcessor".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"ProcessConfig".to_string()));
    assert!(result.metadata.exports.contains(&"Cacheable".to_string()));
    assert!(result.metadata.exports.contains(&"transform".to_string()));

    // Private methods excluded
    assert!(!result
        .metadata
        .exports
        .contains(&"_internal_helper".to_string()));

    // Imports (require)
    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(result.metadata.imports.contains(&"net/http".to_string()));

    // Dependencies (require_relative)
    assert!(result.metadata.dependencies.contains(&"config".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"lib/helpers".to_string()));

    // Mixins (custom fields)
    let fields = result.custom_fields.expect("should have custom fields");
    let mixins = fields
        .get("mixins")
        .expect("should have mixins")
        .as_array()
        .unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Comparable"));
    assert!(mixin_names.contains(&"Enumerable"));

    assert!(result.metadata.loc > 50);
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
