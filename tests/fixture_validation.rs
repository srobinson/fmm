use fmm::parser::builtin::c::CParser;
use fmm::parser::builtin::cpp::CppParser;
use fmm::parser::builtin::csharp::CSharpParser;
use fmm::parser::builtin::go::GoParser;
use fmm::parser::builtin::java::JavaParser;
use fmm::parser::builtin::php::PhpParser;
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::ruby::RubyParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::zig::ZigParser;
use fmm::parser::Parser;

#[test]
fn validate_python_fixture() {
    let source = include_str!("../fixtures/sample.py");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Expected exports from __all__: fetch_data, transform, DataProcessor, ProcessConfig, MAX_RETRIES
    let expected_exports = vec![
        "fetch_data",
        "transform",
        "DataProcessor",
        "ProcessConfig",
        "MAX_RETRIES",
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
        .export_names()
        .contains(&"_internal_helper".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"_INTERNAL_TIMEOUT".to_string()));
}

#[test]
fn validate_rust_fixture() {
    let source = include_str!("../fixtures/sample.rs");

    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Expected exports: pub items only (not pub(crate), pub(super), or private)
    let expected_exports = vec!["Config", "Status", "Pipeline", "Error", "process"];
    assert_eq!(result.metadata.export_names(), expected_exports);

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
        .export_names()
        .contains(&"internal_helper".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"parent_visible".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"private_fn".to_string()));
}

#[test]
fn validate_go_fixture() {
    let source = include_str!("../fixtures/sample.go");

    let mut parser = GoParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Exported: capitalized names only (StatusActive, StatusInactive are iota consts)
    let expected_exports = vec![
        "MaxRetries",
        "Status",
        "StatusActive",
        "StatusInactive",
        "Config",
        "Handler",
        "NewHandler",
        "Process",
    ];
    assert_eq!(result.metadata.export_names(), expected_exports);

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
        .export_names()
        .contains(&"internalTimeout".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"privateState".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"helperFunc".to_string()));

    assert!(result.metadata.loc > 50);
}

#[test]
fn validate_java_fixture() {
    let source = include_str!("../fixtures/sample.java");

    let mut parser = JavaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Top-level classes, interfaces, enums
    assert!(result
        .metadata
        .export_names()
        .contains(&"DataProcessor".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Repository".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Status".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"ProcessConfig".to_string()));

    // Public methods
    assert!(result
        .metadata
        .export_names()
        .contains(&"process".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"transform".to_string()));

    // Private methods should not be exported
    assert!(!result
        .metadata
        .export_names()
        .contains(&"validate".to_string()));

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

    let mut parser = CppParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Classes, structs, enums, functions, templates
    assert!(result
        .metadata
        .export_names()
        .contains(&"Engine".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Config".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Point".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Status".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Pipeline".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"process".to_string()));

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

    let mut parser = CSharpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Public types only
    assert!(result
        .metadata
        .export_names()
        .contains(&"DataService".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"IRepository".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Status".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"ProcessConfig".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Transform".to_string()));

    // Internal class should NOT be exported
    assert!(!result
        .metadata
        .export_names()
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

    let mut parser = RubyParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Classes, modules, top-level methods
    assert!(result
        .metadata
        .export_names()
        .contains(&"DataProcessor".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"ProcessConfig".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Cacheable".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"transform".to_string()));

    // Private methods excluded
    assert!(!result
        .metadata
        .export_names()
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

#[test]
fn validate_php_fixture() {
    let source = include_str!("../fixtures/sample.php");

    let mut parser = PhpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Top-level types: classes, interfaces, traits, enums
    let names = result.metadata.export_names();
    assert!(names.contains(&"UserController".to_string()));
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));
    assert!(names.contains(&"Loggable".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"ProcessConfig".to_string()));

    // Top-level functions
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"processQueue".to_string()));

    // Top-level constants
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"API_VERSION".to_string()));

    // Public methods exported
    assert!(names.contains(&"index".to_string()));
    assert!(names.contains(&"show".to_string()));
    assert!(names.contains(&"store".to_string()));
    assert!(names.contains(&"create".to_string()));
    assert!(names.contains(&"isValid".to_string()));

    // Interface public methods
    assert!(names.contains(&"find".to_string()));
    assert!(names.contains(&"save".to_string()));
    assert!(names.contains(&"delete".to_string()));

    // Trait public methods
    assert!(names.contains(&"cacheKey".to_string()));
    assert!(names.contains(&"clearCache".to_string()));
    assert!(names.contains(&"logAction".to_string()));

    // Private/protected methods NOT exported
    assert!(!names.contains(&"validateInput".to_string()));
    assert!(!names.contains(&"authorize".to_string()));

    // Namespace imports
    assert!(result.metadata.imports.contains(&"App".to_string()));
    assert!(result.metadata.imports.contains(&"Illuminate".to_string()));

    // Dependencies (require/include paths)
    assert!(!result.metadata.dependencies.is_empty());

    // Custom fields: namespaces
    let fields = result.custom_fields.expect("should have custom fields");
    let namespaces = fields
        .get("namespaces")
        .expect("should have namespaces")
        .as_array()
        .unwrap();
    assert!(!namespaces.is_empty());

    // Custom fields: traits_used
    let traits_used = fields
        .get("traits_used")
        .expect("should have traits_used")
        .as_array()
        .unwrap();
    let trait_names: Vec<&str> = traits_used.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(trait_names.contains(&"Cacheable"));
    assert!(trait_names.contains(&"Loggable"));

    assert!(result.metadata.loc > 40);
}

#[test]
fn validate_c_fixture() {
    let source = include_str!("../fixtures/sample.c");

    let mut parser = CParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Macros
    assert!(names.contains(&"MAX_BUFFER_SIZE".to_string()));
    assert!(names.contains(&"MIN".to_string()));
    assert!(names.contains(&"API_VERSION".to_string()));

    // Typedefs
    assert!(names.contains(&"Callback".to_string()));
    assert!(names.contains(&"HashValue".to_string()));

    // Structs and enums
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Result".to_string()));

    // Non-static functions (including pointer-returning ones)
    assert!(names.contains(&"config_init".to_string()));
    assert!(names.contains(&"process_data".to_string()));
    assert!(names.contains(&"config_free".to_string()));
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"get_buffer".to_string()));
    assert!(names.contains(&"compute_hash".to_string()));

    // Static functions should NOT be exported
    assert!(!names.contains(&"validate_input".to_string()));
    assert!(!names.contains(&"log_message".to_string()));

    // System includes → imports
    assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
    assert!(result.metadata.imports.contains(&"stdlib.h".to_string()));
    assert!(result.metadata.imports.contains(&"string.h".to_string()));

    // Local includes → dependencies
    assert!(result
        .metadata
        .dependencies
        .contains(&"config.h".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"utils/helpers.h".to_string()));

    // Custom fields: macros
    let fields = result.custom_fields.expect("should have custom fields");
    let macros = fields
        .get("macros")
        .expect("should have macros")
        .as_array()
        .unwrap();
    let macro_names: Vec<&str> = macros.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(macro_names.contains(&"MAX_BUFFER_SIZE"));
    assert!(macro_names.contains(&"MIN"));
    assert!(macro_names.contains(&"API_VERSION"));

    // Custom fields: typedefs
    let typedefs = fields
        .get("typedefs")
        .expect("should have typedefs")
        .as_array()
        .unwrap();
    let typedef_names: Vec<&str> = typedefs.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(typedef_names.contains(&"Callback"));
    assert!(typedef_names.contains(&"HashValue"));

    // LOC
    assert!(result.metadata.loc > 40);

    // Exports should be sorted by line number
    let lines: Vec<usize> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.start_line)
        .collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);
}

#[test]
fn validate_zig_fixture() {
    let source = include_str!("../fixtures/sample.zig");

    let mut parser = ZigParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Pub const values
    assert!(names.contains(&"MAX_RETRIES".to_string()));

    // Pub var
    assert!(names.contains(&"debug_enabled".to_string()));

    // Pub const types (struct, enum, error, union)
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"PipelineError".to_string()));
    assert!(names.contains(&"ArrayList".to_string()));
    assert!(names.contains(&"Value".to_string()));

    // Pub functions
    assert!(names.contains(&"processBatch".to_string()));
    assert!(names.contains(&"transform".to_string()));

    // Non-pub items should NOT be exported
    assert!(!names.contains(&"internal_timeout".to_string()));
    assert!(!names.contains(&"internalHelper".to_string()));
    assert!(!names.contains(&"validateInput".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"builtin".to_string()));

    // Dependencies (relative imports)
    assert!(result
        .metadata
        .dependencies
        .contains(&"./utils.zig".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"../config.zig".to_string()));

    // Custom fields: comptime_blocks and test_blocks
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(
        fields.get("comptime_blocks").unwrap().as_u64().unwrap(),
        2,
        "should have 2 comptime blocks"
    );
    assert_eq!(
        fields.get("test_blocks").unwrap().as_u64().unwrap(),
        3,
        "should have 3 test blocks"
    );

    // LOC
    assert!(result.metadata.loc > 40);

    // Exports should be sorted by line number
    let lines: Vec<usize> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.start_line)
        .collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);
}
