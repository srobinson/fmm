use super::*;
use crate::parser::ExportEntry;
use std::path::Path;

#[test]
fn parse_rust_pub_functions() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() {}\nfn private() {}\npub fn world() {}";
    let result = parser.parse(source).unwrap();
    assert!(result
        .metadata
        .export_names()
        .contains(&"hello".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"world".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"private".to_string()));
}

#[test]
fn parse_rust_pub_structs_and_enums() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Foo {}\npub enum Bar { A, B }\nstruct Private {}";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
    assert!(result.metadata.export_names().contains(&"Bar".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"Private".to_string()));
}

#[test]
fn parse_rust_use_imports() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::collections::HashMap;\nuse anyhow::Result;\nuse crate::config::Config;";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(!result.metadata.imports.contains(&"crate".to_string()));
}

#[test]
fn parse_rust_extern_crate() {
    let mut parser = RustParser::new().unwrap();
    let source = "extern crate serde;\nextern crate log;\nuse serde::Deserialize;";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));
}

#[test]
fn parse_rust_includes_std_core_alloc() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::io;\nuse core::fmt;\nuse alloc::vec::Vec;\nuse tokio::runtime;";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"core".to_string()));
    assert!(result.metadata.imports.contains(&"alloc".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));
}

#[test]
fn parse_rust_pub_crate_excluded() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn visible() {}\npub(crate) fn internal() {}\npub(super) fn parent_only() {}";
    let result = parser.parse(source).unwrap();
    assert!(result
        .metadata
        .export_names()
        .contains(&"visible".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"internal".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"parent_only".to_string()));
}

#[test]
fn parse_rust_crate_deps() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::config::Config;\nuse super::utils;";
    let result = parser.parse(source).unwrap();
    let deps = &result.metadata.dependencies;
    // Full paths, not bare root keywords
    assert!(
        deps.contains(&"crate::config".to_string()),
        "expected crate::config in {:?}",
        deps
    );
    assert!(
        deps.contains(&"../utils".to_string()),
        "expected ../utils in {:?}",
        deps
    );
    // External stdlib stays out of deps
    assert!(!deps.contains(&"std".to_string()));
}

#[test]
fn rust_use_path_to_dep_conversions() {
    assert_eq!(
        rust_use_path_to_dep("crate::config::Config"),
        Some("crate::config".into())
    );
    assert_eq!(
        rust_use_path_to_dep("crate::parser::builtin::rust"),
        Some("crate::parser::builtin::rust".into())
    );
    assert_eq!(
        rust_use_path_to_dep("super::utils"),
        Some("../utils".into())
    );
    assert_eq!(
        rust_use_path_to_dep("super::parser::builtin"),
        Some("../parser/builtin".into())
    );
    assert_eq!(rust_use_path_to_dep("std::collections::HashMap"), None);
    assert_eq!(rust_use_path_to_dep("anyhow"), None);
}

#[test]
fn rust_custom_fields_unsafe() {
    let mut parser = RustParser::new().unwrap();
    let source = "fn foo() { unsafe { std::ptr::null() }; }\nfn bar() { unsafe { 1 }; }";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("unsafe_blocks").unwrap().as_u64().unwrap(), 2);
}

#[test]
fn rust_custom_fields_derives() {
    let mut parser = RustParser::new().unwrap();
    let source = "#[derive(Debug, Clone, Serialize)]\npub struct Foo {}";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Debug"));
    assert!(names.contains(&"Clone"));
    assert!(names.contains(&"Serialize"));
}

#[test]
fn rust_no_custom_fields_when_clean() {
    let mut parser = RustParser::new().unwrap();
    // No pub functions, no unsafe, no derives, no traits, no lifetimes, no async
    let source = "pub struct Empty {}";
    let result = parser.parse(source).unwrap();
    assert!(result.custom_fields.is_none());
}

#[test]
fn parse_rust_loc() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn hello() {\n    42\n}\n";
    let result = parser.parse(source).unwrap();
    assert_eq!(result.metadata.loc, 3);
}

#[test]
fn rust_custom_fields_trait_impls() {
    let mut parser = RustParser::new().unwrap();
    let source = "struct Foo {}\nimpl Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n}\nimpl Clone for Foo {\n    fn clone(&self) -> Self { Foo {} }\n}";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Clone for Foo"));
    assert!(names.contains(&"Display for Foo"));
}

#[test]
fn rust_custom_fields_lifetimes() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Ref<'a> {\n    data: &'a str,\n}";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
    let names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"'a"));
}

#[test]
fn rust_custom_fields_async_functions() {
    let mut parser = RustParser::new().unwrap();
    let source = "async fn fetch() {}\nasync fn process() {}\nfn sync_fn() {}";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("async_functions").unwrap().as_u64().unwrap(), 2);
}

#[test]
fn rust_scoped_trait_impl() {
    let mut parser = RustParser::new().unwrap();
    let source = "struct Foo {}\nimpl std::fmt::Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }\n}";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Display for Foo"));
}

#[test]
fn rust_impl_methods_get_own_line_ranges() {
    let mut parser = RustParser::new().unwrap();
    let source = "\
pub struct Foo {
    x: i32,
}

impl Foo {
    pub fn new(x: i32) -> Self {
        Self { x }
    }

    pub fn get_x(&self) -> i32 {
        self.x
    }
}";
    let result = parser.parse(source).unwrap();
    let exports = &result.metadata.exports;

    let foo = exports.iter().find(|e| e.name == "Foo").unwrap();
    assert_eq!(foo.start_line, 1);
    assert_eq!(foo.end_line, 3);

    let new_fn = exports.iter().find(|e| e.name == "new").unwrap();
    assert_eq!(new_fn.start_line, 6);
    assert_eq!(new_fn.end_line, 8);

    let get_x = exports.iter().find(|e| e.name == "get_x").unwrap();
    assert_eq!(get_x.start_line, 10);
    assert_eq!(get_x.end_line, 12);

    // Verify sorted by line number
    assert!(exports[0].start_line <= exports[1].start_line);
    assert!(exports[1].start_line <= exports[2].start_line);
}

#[test]
fn binary_main_exports_all_functions() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
fn main() {
    run();
}

fn run() {}

fn helper() -> i32 { 42 }

struct Config {
    name: String,
}

enum Mode { Fast, Slow }

const VERSION: &str = "1.0";
"#;
    let result = parser.parse_file(source, Path::new("src/main.rs")).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"run".to_string()));
    assert!(names.contains(&"helper".to_string()));
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Mode".to_string()));
    assert!(names.contains(&"VERSION".to_string()));
}

#[test]
fn binary_bin_dir_exports_all_functions() {
    let mut parser = RustParser::new().unwrap();
    let source = "fn main() {}\nfn setup() {}";
    let result = parser
        .parse_file(source, Path::new("src/bin/tool.rs"))
        .unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"setup".to_string()));
}

#[test]
fn lib_still_requires_pub() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn visible() {}\nfn private() {}\npub struct Exported {}\nstruct Hidden {}";
    let result = parser.parse_file(source, Path::new("src/lib.rs")).unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"visible".to_string()));
    assert!(names.contains(&"Exported".to_string()));
    assert!(!names.contains(&"private".to_string()));
    assert!(!names.contains(&"Hidden".to_string()));
}

// ALP-770: impl block method extraction tests

fn get_method<'a>(
    exports: &'a [ExportEntry],
    class: &str,
    method: &str,
) -> Option<&'a ExportEntry> {
    exports
        .iter()
        .find(|e| e.parent_class.as_deref() == Some(class) && e.name == method)
}

#[test]
fn rust_impl_pub_fn_indexed_as_method() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn bar() {}\n}";
    let result = parser.parse(source).unwrap();
    let entry =
        get_method(&result.metadata.exports, "Foo", "bar").expect("Foo.bar should be indexed");
    assert_eq!(entry.parent_class.as_deref(), Some("Foo"));
}

#[test]
fn rust_impl_private_fn_not_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Foo;\nimpl Foo {\n    fn internal() {}\n}";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "internal").is_none(),
        "Foo.internal (no pub) should NOT be indexed"
    );
}

#[test]
fn rust_trait_impl_pub_fn_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source =
        "pub struct Foo;\ntrait MyTrait {\n    fn method(&self);\n}\nimpl MyTrait for Foo {\n    pub fn method(&self) {}\n}";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "method").is_some(),
        "Foo.method from trait impl should be indexed"
    );
}

#[test]
fn rust_impl_non_exported_struct_not_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = "struct Hidden;\nimpl Hidden {\n    pub fn method() {}\n}";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Hidden", "method").is_none(),
        "methods of non-exported struct should NOT be indexed"
    );
}

#[test]
fn rust_impl_method_line_range_spans_full_body() {
    let mut parser = RustParser::new().unwrap();
    // line 1: pub struct Foo;
    // line 2: impl Foo {
    // line 3:     pub fn bar() {
    // line 4:         42
    // line 5:     }
    // line 6: }
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn bar() {\n        42\n    }\n}";
    let result = parser.parse(source).unwrap();
    let entry =
        get_method(&result.metadata.exports, "Foo", "bar").expect("Foo.bar should be indexed");
    assert_eq!(entry.start_line, 3);
    assert_eq!(entry.end_line, 5);
}

#[test]
fn rust_impl_generic_type_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Wrapper<T>(T);\nimpl<T> Wrapper<T> {\n    pub fn inner(&self) -> &T { &self.0 }\n}";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Wrapper", "inner").is_some(),
        "Wrapper<T>.inner should be indexed under Wrapper"
    );
}

#[test]
fn rust_impl_methods_have_correct_parent_class() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn new() -> Self { Foo }\n    pub fn get_x(&self) -> i32 { 0 }\n}";
    let result = parser.parse(source).unwrap();
    let exports = &result.metadata.exports;

    let new_fn = get_method(exports, "Foo", "new").expect("Foo.new should be indexed");
    assert_eq!(new_fn.parent_class.as_deref(), Some("Foo"));

    let get_x = get_method(exports, "Foo", "get_x").expect("Foo.get_x should be indexed");
    assert_eq!(get_x.parent_class.as_deref(), Some("Foo"));

    // Foo itself should still be a top-level export
    let foo = exports
        .iter()
        .find(|e| e.name == "Foo" && e.parent_class.is_none())
        .expect("Foo should be a top-level export");
    assert_eq!(foo.start_line, 1);
}

#[test]
fn rust_anonymous_lifetime_filtered() {
    let mut parser = RustParser::new().unwrap();
    let source = "fn foo(x: &'_ str) {}";
    let result = parser.parse(source).unwrap();
    if let Some(fields) = result.custom_fields {
        if let Some(lts) = fields.get("lifetimes") {
            let names: Vec<&str> = lts
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert!(!names.contains(&"'_"));
        }
    }
}

#[test]
fn pub_use_simple_path_indexes_rightmost_segment() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use crate::runtime::Runtime;";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Runtime".to_string()),
        "expected Runtime in {:?}",
        names
    );
}

#[test]
fn pub_use_alias_indexes_alias_name() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use crate::runtime::Runtime as Rt;";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Rt".to_string()),
        "expected Rt in {:?}",
        names
    );
    assert!(
        !names.contains(&"Runtime".to_string()),
        "Runtime should not appear (aliased)"
    );
}

#[test]
fn pub_use_grouped_indexes_each_name() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use crate::task::{JoinHandle, LocalSet};";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"JoinHandle".to_string()),
        "expected JoinHandle in {:?}",
        names
    );
    assert!(
        names.contains(&"LocalSet".to_string()),
        "expected LocalSet in {:?}",
        names
    );
}

#[test]
fn pub_use_grouped_with_alias_indexes_alias() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use crate::task::{JoinHandle as JH};";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"JH".to_string()),
        "expected JH in {:?}",
        names
    );
    assert!(
        !names.contains(&"JoinHandle".to_string()),
        "JoinHandle should not appear (aliased)"
    );
}

// ---- ALP-776: wildcard use as dependency ----

#[test]
fn wildcard_use_crate_module_recorded_as_dep() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::parser::*;";
    let result = parser.parse(source).unwrap();
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"crate::parser".to_string()),
        "expected crate::parser in deps {:?}",
        deps
    );
}

#[test]
fn wildcard_use_super_module_recorded_as_dep() {
    let mut parser = RustParser::new().unwrap();
    let source = "use super::utils::*;";
    let result = parser.parse(source).unwrap();
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"../utils".to_string()),
        "expected ../utils in deps {:?}",
        deps
    );
}

#[test]
fn wildcard_use_external_crate_not_a_dep() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::io::*;";
    let result = parser.parse(source).unwrap();
    let deps = &result.metadata.dependencies;
    assert!(
        deps.is_empty(),
        "std wildcard should produce no local dep, got {:?}",
        deps
    );
    // But it should appear in imports
    assert!(
        result.metadata.imports.contains(&"std".to_string()),
        "std should be in imports"
    );
}

#[test]
fn pub_use_wildcard_skipped() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use crate::prelude::*;";
    let result = parser.parse(source).unwrap();
    // No exports should be emitted for wildcard re-exports
    assert!(
        result.metadata.exports.is_empty(),
        "wildcard pub use should emit no exports, got {:?}",
        result.metadata.export_names()
    );
}

#[test]
fn non_pub_use_not_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::runtime::Runtime;";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"Runtime".to_string()),
        "non-pub use should not be indexed"
    );
}

#[test]
fn pub_use_external_crate_indexes_rightmost() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub use tokio_util::codec::Framed;";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Framed".to_string()),
        "expected Framed in {:?}",
        names
    );
}

#[test]
fn pub_crate_use_not_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub(crate) use crate::runtime::Runtime;";
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"Runtime".to_string()),
        "pub(crate) use should not be indexed as a public export"
    );
}

// ---- ALP-775: macro_export and proc-macro indexing ----

#[test]
fn macro_export_indexed_with_bang_suffix() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[macro_export]
macro_rules! select {
    ($($t:tt)*) => {};
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"select!".to_string()),
        "expected select! in {:?}",
        names
    );
}

#[test]
fn macro_rules_without_macro_export_not_indexed() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
macro_rules! internal {
    () => {};
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"internal!".to_string()),
        "internal macro should not be indexed"
    );
}

#[test]
fn macro_export_with_multiple_preceding_attrs() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[doc(hidden)]
#[macro_export]
macro_rules! join {
    () => {};
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"join!".to_string()),
        "expected join! when #[macro_export] is not the first attr: {:?}",
        names
    );
}

#[test]
fn proc_macro_derive_indexes_derive_name() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[proc_macro_derive(Serialize)]
pub fn derive_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Serialize".to_string()),
        "expected Serialize in {:?}",
        names
    );
}

#[test]
fn proc_macro_derive_with_attributes_arg_indexes_derive_name_only() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn derive_deserialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Deserialize".to_string()),
        "expected Deserialize in {:?}",
        names
    );
    assert!(
        !names.contains(&"serde".to_string()),
        "attributes argument should not be indexed"
    );
}

#[test]
fn proc_macro_attribute_indexes_function_name() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[proc_macro_attribute]
pub fn route(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"route".to_string()),
        "expected route in {:?}",
        names
    );
}

#[test]
fn proc_macro_function_indexes_function_name() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
#[proc_macro]
pub fn my_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"my_macro".to_string()),
        "expected my_macro in {:?}",
        names
    );
}

// --- ALP-1419: named_imports and namespace_imports ---

#[test]
fn named_imports_scoped_identifier() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::collections::HashMap;\nuse anyhow::Result;";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "use std::collections::HashMap -> named_imports; got: {ni:?}"
    );
    assert_eq!(
        ni.get("anyhow").map(Vec::as_slice),
        Some(vec!["Result".to_string()].as_slice()),
        "use anyhow::Result -> named_imports; got: {ni:?}"
    );
}

#[test]
fn named_imports_grouped() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::collections::{HashMap, BTreeMap};";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    let names = ni
        .get("std::collections")
        .expect("should have std::collections");
    assert!(names.contains(&"HashMap".to_string()), "got: {names:?}");
    assert!(names.contains(&"BTreeMap".to_string()), "got: {names:?}");
}

#[test]
fn named_imports_crate_path() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::parser::Metadata;";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("crate::parser").map(Vec::as_slice),
        Some(vec!["Metadata".to_string()].as_slice()),
        "use crate::parser::Metadata -> named_imports; got: {ni:?}"
    );
}

#[test]
fn named_imports_aliased_stores_original() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::collections::HashMap as Map;";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "aliased import should store original name; got: {ni:?}"
    );
}

#[test]
fn namespace_imports_wildcard() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::parser::*;";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"crate::parser".to_string()),
        "use crate::parser::* -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    assert!(
        result.metadata.named_imports.is_empty(),
        "wildcard should not populate named_imports"
    );
}

#[test]
fn named_imports_nested_groups() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::{collections::HashMap, io::Read};";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "nested group std::collections::HashMap; got: {ni:?}"
    );
    assert_eq!(
        ni.get("std::io").map(Vec::as_slice),
        Some(vec!["Read".to_string()].as_slice()),
        "nested group std::io::Read; got: {ni:?}"
    );
}

#[test]
fn named_imports_self_in_group() {
    let mut parser = RustParser::new().unwrap();
    let source = "use crate::parser::{self, Metadata};";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"crate::parser".to_string()),
        "self in group -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("crate::parser").map(Vec::as_slice),
        Some(vec!["Metadata".to_string()].as_slice()),
        "Metadata alongside self; got: {ni:?}"
    );
}

#[test]
fn named_imports_aliased_in_group() {
    let mut parser = RustParser::new().unwrap();
    let source = "use std::collections::{HashMap as Map, BTreeMap};";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    let names = ni
        .get("std::collections")
        .expect("should have std::collections");
    assert!(
        names.contains(&"HashMap".to_string()),
        "aliased in group stores original; got: {names:?}"
    );
    assert!(
        names.contains(&"BTreeMap".to_string()),
        "non-aliased sibling; got: {names:?}"
    );
}

#[test]
fn named_imports_super_path() {
    let mut parser = RustParser::new().unwrap();
    let source = "use super::utils::Helper;";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("super::utils").map(Vec::as_slice),
        Some(vec!["Helper".to_string()].as_slice()),
        "use super::utils::Helper; got: {ni:?}"
    );
}

#[test]
fn named_imports_mixed_forms() {
    let mut parser = RustParser::new().unwrap();
    let source = r#"
use std::collections::HashMap;
use crate::parser::{Metadata, ExportEntry};
use crate::config::*;
use anyhow::Result as AnyhowResult;
"#;
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    let ns = &result.metadata.namespace_imports;

    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
    );
    let parser_names = ni.get("crate::parser").expect("crate::parser");
    assert!(parser_names.contains(&"Metadata".to_string()));
    assert!(parser_names.contains(&"ExportEntry".to_string()));
    assert!(ns.contains(&"crate::config".to_string()));
    assert_eq!(
        ni.get("anyhow").map(Vec::as_slice),
        Some(vec!["Result".to_string()].as_slice()),
    );
}

// --- ALP-1423: function_names custom field ---

#[test]
fn function_names_populated() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn foo() {}\npub fn bar() {}\npub struct Baz {}\n";
    let result = parser.parse(source).unwrap();
    let cf = result.custom_fields.expect("custom_fields should be Some");
    let fn_names = cf.get("function_names").expect("function_names key");
    let names: Vec<&str> = fn_names
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(names.contains(&"foo"), "foo missing: {names:?}");
    assert!(names.contains(&"bar"), "bar missing: {names:?}");
    assert!(
        !names.contains(&"Baz"),
        "struct should not be in function_names: {names:?}"
    );
}

#[test]
fn function_names_excludes_private() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub fn exported() {}\nfn private_helper() {}\n";
    let result = parser.parse(source).unwrap();
    let cf = result.custom_fields.expect("custom_fields should be Some");
    let fn_names = cf.get("function_names").expect("function_names key");
    let names: Vec<&str> = fn_names
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(names.contains(&"exported"), "exported missing: {names:?}");
    assert!(
        !names.contains(&"private_helper"),
        "private should be excluded: {names:?}"
    );
}

#[test]
fn function_names_empty_for_no_functions() {
    let mut parser = RustParser::new().unwrap();
    let source = "pub struct Foo {}\npub enum Bar { A, B }\n";
    let result = parser.parse(source).unwrap();
    let has_fn = result
        .custom_fields
        .as_ref()
        .and_then(|cf| cf.get("function_names"))
        .is_some();
    assert!(!has_fn, "no functions should mean no function_names key");
}
