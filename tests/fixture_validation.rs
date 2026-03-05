use fmm::parser::builtin::c::CParser;
use fmm::parser::builtin::cpp::CppParser;
use fmm::parser::builtin::csharp::CSharpParser;
use fmm::parser::builtin::dart::DartParser;
use fmm::parser::builtin::elixir::ElixirParser;
use fmm::parser::builtin::go::GoParser;
use fmm::parser::builtin::java::JavaParser;
use fmm::parser::builtin::kotlin::KotlinParser;
use fmm::parser::builtin::lua::LuaParser;
use fmm::parser::builtin::php::PhpParser;
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::ruby::RubyParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::scala::ScalaParser;
use fmm::parser::builtin::swift::SwiftParser;
use fmm::parser::builtin::zig::ZigParser;
use fmm::parser::Parser;

#[test]
fn validate_python_fixture() {
    let source = include_str!("../fixtures/sample.py");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

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
fn validate_python_heuristic_fixture() {
    let source = include_str!("../fixtures/python/heuristic.py");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

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

    // Private items excluded
    assert!(!names.contains(&"_internal_setup".to_string()));
    assert!(!names.contains(&"_Registry".to_string()));

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
    let source = include_str!("../fixtures/python/decorated.py");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();
    assert!(names.contains(&"SimpleDecorated".to_string()));
    assert!(names.contains(&"DecoratedWithArgs".to_string()));
    assert!(names.contains(&"multi_decorated".to_string()));
    assert!(names.contains(&"bare_function".to_string()));
    assert!(names.contains(&"BareClass".to_string()));

    // Private decorated class excluded
    assert!(!names.contains(&"_PrivateDecorated".to_string()));

    // Decorators captured in custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let deco_names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(deco_names.contains(&"dataclass"));
    assert!(deco_names.contains(&"lru_cache"));
    assert!(deco_names.contains(&"staticmethod"));
}

#[test]
fn validate_python_with_all_fixture() {
    let source = include_str!("../fixtures/python/with_all.py");

    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

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

#[test]
fn validate_rust_fixture() {
    let source = include_str!("../fixtures/sample.rs");

    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Expected exports: pub items only (not pub(crate), pub(super), or private)
    let expected_exports = vec!["Config", "Status", "Pipeline", "Error", "process"];
    assert_eq!(result.metadata.export_names(), expected_exports);

    // Expected imports: anyhow, serde, std, tokio (all crates including stdlib)
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));

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

    // Classes, structs, enums, functions, templates with correct declaration line ranges
    let exports = &result.metadata.exports;
    let find = |name: &str| exports.iter().find(|e| e.name == name).unwrap();

    let point = find("Point");
    assert_eq!((point.start_line, point.end_line), (10, 12));

    let status = find("Status");
    assert_eq!((status.start_line, status.end_line), (14, 18));

    let config = find("Config");
    assert_eq!((config.start_line, config.end_line), (20, 28));

    let engine = find("Engine");
    assert_eq!((engine.start_line, engine.end_line), (30, 39));

    let pipeline = find("Pipeline");
    assert_eq!((pipeline.start_line, pipeline.end_line), (41, 54)); // template_declaration

    let process = find("process");
    assert_eq!((process.start_line, process.end_line), (60, 64));

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

    // Exports should be sorted by line number
    let lines: Vec<usize> = exports.iter().map(|e| e.start_line).collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);

    assert!(result.metadata.loc > 50);
}

#[test]
fn validate_csharp_fixture() {
    let source = include_str!("../fixtures/sample.cs");

    let mut parser = CSharpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Public types with correct declaration line ranges (not namespace line ranges)
    let exports = &result.metadata.exports;
    let find = |name: &str| exports.iter().find(|e| e.name == name).unwrap();

    let ds = find("DataService");
    assert_eq!((ds.start_line, ds.end_line), (8, 29)); // includes [Serializable] attribute

    let transform = find("Transform");
    assert_eq!((transform.start_line, transform.end_line), (18, 22)); // includes [Required] attribute

    let repo = find("IRepository");
    assert_eq!((repo.start_line, repo.end_line), (31, 36));

    let status = find("Status");
    assert_eq!((status.start_line, status.end_line), (38, 44)); // includes [Obsolete] attribute

    let config = find("ProcessConfig");
    assert_eq!((config.start_line, config.end_line), (49, 54));

    // Internal class should NOT be exported
    assert!(!result
        .metadata
        .export_names()
        .contains(&"InternalHelper".to_string()));

    // Exports should be sorted by line number
    let lines: Vec<usize> = exports.iter().map(|e| e.start_line).collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);

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

#[test]
fn validate_lua_fixture() {
    let source = include_str!("../fixtures/sample.lua");

    let mut parser = LuaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Module methods (M.name pattern)
    assert!(names.contains(&"init".to_string()));
    assert!(names.contains(&"process".to_string()));
    assert!(names.contains(&"transform".to_string()));
    assert!(names.contains(&"status".to_string()));
    assert!(names.contains(&"reset".to_string()));

    // Global functions
    assert!(names.contains(&"create_connection".to_string()));
    assert!(names.contains(&"parse_config".to_string()));

    // Local functions should NOT be exported
    assert!(!names.contains(&"validate_input".to_string()));
    assert!(!names.contains(&"format_output".to_string()));
    assert!(!names.contains(&"log_action".to_string()));

    // Imports (require calls with non-relative paths)
    assert!(result.metadata.imports.contains(&"cjson".to_string()));
    assert!(result.metadata.imports.contains(&"socket".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));

    // Dependencies (require calls with relative paths)
    assert!(result
        .metadata
        .dependencies
        .contains(&"./config".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"../lib/utils".to_string()));

    // No custom fields for Lua
    assert!(result.custom_fields.is_none());

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
fn validate_scala_fixture() {
    let source = include_str!("../fixtures/sample.scala");

    let mut parser = ScalaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Case classes
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Status".to_string()));
    assert!(names.contains(&"Success".to_string()));
    assert!(names.contains(&"Failure".to_string()));

    // Traits
    assert!(names.contains(&"Processor".to_string()));
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Result".to_string()));

    // Classes
    assert!(names.contains(&"DataService".to_string()));
    assert!(names.contains(&"LegacyProcessor".to_string()));

    // Objects
    assert!(names.contains(&"DataService".to_string()));
    assert!(names.contains(&"Pipeline".to_string()));

    // Top-level function
    assert!(names.contains(&"transform".to_string()));

    // Top-level val/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"globalState".to_string()));

    // Implicit def
    assert!(names.contains(&"stringToConfig".to_string()));

    // Private items should NOT be exported
    assert!(!names.contains(&"InternalHelper".to_string()));
    assert!(!names.contains(&"InternalUtils".to_string()));

    // Imports (root packages)
    assert!(result.metadata.imports.contains(&"scala".to_string()));
    assert!(result.metadata.imports.contains(&"akka".to_string()));
    assert!(result.metadata.imports.contains(&"com".to_string()));

    // Custom fields: case_classes
    let fields = result.custom_fields.expect("should have custom fields");
    let cc = fields
        .get("case_classes")
        .expect("should have case_classes")
        .as_array()
        .unwrap();
    let cc_names: Vec<&str> = cc.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(cc_names.contains(&"Config"));
    assert!(cc_names.contains(&"Status"));
    assert!(cc_names.contains(&"Success"));
    assert!(cc_names.contains(&"Failure"));

    // Custom fields: implicits
    assert_eq!(
        fields.get("implicits").unwrap().as_u64().unwrap(),
        1,
        "should have 1 implicit definition"
    );

    // Custom fields: annotations
    let annotations = fields
        .get("annotations")
        .expect("should have annotations")
        .as_array()
        .unwrap();
    let ann_names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ann_names.contains(&"deprecated"));
    assert!(ann_names.contains(&"volatile"));

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

// =============================================================================
// Swift fixture validation
// =============================================================================

#[test]
fn validate_swift_fixture() {
    let source = include_str!("../fixtures/sample.swift");

    let mut parser = SwiftParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Public classes
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseViewController".to_string()));

    // Public structs
    assert!(names.contains(&"Point".to_string()));
    assert!(names.contains(&"APIConfig".to_string()));

    // Public enums
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"NetworkError".to_string()));

    // Protocols
    assert!(names.contains(&"Drawable".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Top-level public function
    assert!(names.contains(&"createManager".to_string()));

    // Public let/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));

    // Public typealias
    assert!(names.contains(&"JSONDictionary".to_string()));
    assert!(names.contains(&"CompletionHandler".to_string()));

    // Public extension methods
    assert!(names.contains(&"trimmed".to_string()));
    assert!(names.contains(&"uniqueElements".to_string()));

    // Internal/private/fileprivate/default should NOT be exported
    assert!(!names.contains(&"InternalConfig".to_string()));
    assert!(!names.contains(&"helperFunction".to_string()));
    assert!(!names.contains(&"secretFunction".to_string()));
    assert!(!names.contains(&"DefaultAccessStruct".to_string()));
    assert!(!names.contains(&"defaultAccessFunc".to_string()));
    assert!(!names.contains(&"internalRetry".to_string()));
    assert!(!names.contains(&"defaultAccessMethod".to_string()));

    // Non-public extension methods should NOT be exported
    assert!(!names.contains(&"doubled".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"Foundation".to_string()));
    assert!(result.metadata.imports.contains(&"UIKit".to_string()));
    assert!(result
        .metadata
        .imports
        .contains(&"MyTestModule".to_string()));

    // Dependencies (Swift has none)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(
        fields.get("protocols").unwrap().as_u64().unwrap(),
        2,
        "should have 2 protocol declarations"
    );
    assert_eq!(
        fields.get("extensions").unwrap().as_u64().unwrap(),
        3,
        "should have 3 extension declarations"
    );

    // LOC
    assert!(result.metadata.loc > 100);

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

// =============================================================================
// Kotlin fixture validation
// =============================================================================

#[test]
fn validate_kotlin_fixture() {
    let source = include_str!("../fixtures/sample.kt");

    let mut parser = KotlinParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Classes (default public)
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseRepository".to_string()));
    assert!(names.contains(&"ServiceLocator".to_string()));

    // Data classes
    assert!(names.contains(&"UserProfile".to_string()));
    assert!(names.contains(&"APIResponse".to_string()));

    // Sealed class
    assert!(names.contains(&"Result".to_string()));

    // Interfaces
    assert!(names.contains(&"Repository".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Objects
    assert!(names.contains(&"AppConfig".to_string()));
    assert!(names.contains(&"DatabaseManager".to_string()));

    // Enum classes
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"HttpStatus".to_string()));

    // Top-level functions
    assert!(names.contains(&"createManager".to_string()));
    assert!(names.contains(&"processData".to_string()));
    assert!(names.contains(&"oldMethod".to_string()));
    assert!(names.contains(&"asyncOperation".to_string()));

    // Top-level val/var
    assert!(names.contains(&"MAX_RETRIES".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));
    assert!(names.contains(&"VERSION".to_string()));

    // Typealias
    assert!(names.contains(&"StringMap".to_string()));
    assert!(names.contains(&"Callback".to_string()));

    // Private/internal should NOT be exported
    assert!(!names.contains(&"InternalHelper".to_string()));
    assert!(!names.contains(&"ModuleInternal".to_string()));
    assert!(!names.contains(&"hiddenFunction".to_string()));
    assert!(!names.contains(&"moduleFunction".to_string()));

    // Imports (package roots — first two segments)
    assert!(result
        .metadata
        .imports
        .contains(&"kotlin.collections".to_string()));
    assert!(result.metadata.imports.contains(&"java.util".to_string()));
    assert!(result.metadata.imports.contains(&"org.example".to_string()));

    // Dependencies (empty for Kotlin)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(
        fields.get("data_classes").unwrap().as_u64().unwrap(),
        2,
        "should have 2 data class declarations"
    );
    assert_eq!(
        fields.get("sealed_classes").unwrap().as_u64().unwrap(),
        1,
        "should have 1 sealed class declaration"
    );
    assert_eq!(
        fields.get("companion_objects").unwrap().as_u64().unwrap(),
        1,
        "should have 1 companion object"
    );

    // LOC
    assert!(result.metadata.loc > 80);

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
fn validate_dart_fixture() {
    let source = include_str!("../fixtures/sample.dart");
    let mut parser = DartParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Public classes
    assert!(names.contains(&"NetworkManager".to_string()));
    assert!(names.contains(&"BaseWidget".to_string()));
    assert!(names.contains(&"UserProfile".to_string()));
    // Private class excluded
    assert!(!names.contains(&"_InternalHelper".to_string()));

    // Mixins
    assert!(names.contains(&"Loggable".to_string()));
    assert!(names.contains(&"Cacheable".to_string()));

    // Enums
    assert!(names.contains(&"Direction".to_string()));
    assert!(names.contains(&"HttpStatus".to_string()));

    // Extensions
    assert!(names.contains(&"StringExtension".to_string()));
    assert!(names.contains(&"IntExtension".to_string()));

    // Typedefs
    assert!(names.contains(&"Callback".to_string()));
    assert!(names.contains(&"JsonMap".to_string()));
    assert!(!names.contains(&"_PrivateCallback".to_string()));

    // Public functions
    assert!(names.contains(&"globalFunction".to_string()));
    assert!(names.contains(&"processData".to_string()));
    assert!(names.contains(&"asyncOperation".to_string()));
    assert!(!names.contains(&"_privateFunction".to_string()));

    // Top-level variables
    assert!(names.contains(&"appVersion".to_string()));
    assert!(names.contains(&"maxRetries".to_string()));
    assert!(names.contains(&"isDebugMode".to_string()));
    assert!(!names.contains(&"_privateVar".to_string()));

    // Imports (package names)
    assert!(result.metadata.imports.contains(&"flutter".to_string()));
    assert!(result.metadata.imports.contains(&"http".to_string()));
    assert!(result.metadata.imports.contains(&"dart:async".to_string()));
    assert!(result
        .metadata
        .imports
        .contains(&"dart:convert".to_string()));

    // Dependencies (relative paths)
    assert!(result
        .metadata
        .dependencies
        .contains(&"./relative_file.dart".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"../utils/helpers.dart".to_string()));

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(fields.get("mixins").unwrap().as_u64().unwrap(), 2);
    assert_eq!(fields.get("extensions").unwrap().as_u64().unwrap(), 2);

    // LOC
    assert!(result.metadata.loc >= 100);

    // Exports sorted by line number
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
fn validate_elixir_fixture() {
    let source = include_str!("../fixtures/sample.ex");
    let mut parser = ElixirParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Modules
    assert!(names.contains(&"MyApp.Router".to_string()));
    assert!(names.contains(&"MyApp.Helpers".to_string()));
    assert!(names.contains(&"MyApp.Config".to_string()));

    // Public functions
    assert!(names.contains(&"handle".to_string()));
    assert!(names.contains(&"helper_function".to_string()));
    assert!(names.contains(&"another_helper".to_string()));
    assert!(names.contains(&"get".to_string()));
    assert!(names.contains(&"set".to_string()));

    // Private functions excluded
    assert!(!names.contains(&"private_handler".to_string()));
    assert!(!names.contains(&"internal_work".to_string()));

    // Public macros
    assert!(names.contains(&"route".to_string()));
    assert!(!names.contains(&"private_macro".to_string()));

    // Public guards
    assert!(names.contains(&"is_valid".to_string()));
    assert!(!names.contains(&"is_internal".to_string()));

    // Delegates
    assert!(names.contains(&"format".to_string()));

    // Protocols
    assert!(names.contains(&"Printable".to_string()));
    assert!(names.contains(&"print".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"Plug".to_string()));
    assert!(result.metadata.imports.contains(&"Logger".to_string()));
    assert!(result.metadata.imports.contains(&"MyApp".to_string()));
    assert!(result.metadata.imports.contains(&"EEx".to_string()));
    assert!(result.metadata.imports.contains(&"GenServer".to_string()));
    assert!(result.metadata.imports.contains(&"Enum".to_string()));

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(fields.get("macros").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("protocols").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("behaviours").unwrap().as_u64().unwrap(), 1);

    // LOC
    assert!(result.metadata.loc >= 73);

    // Exports sorted by line number
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
