use super::support::parse;

#[test]
fn macro_export_indexed_with_bang_suffix() {
    let source = r#"
#[macro_export]
macro_rules! select {
    ($($t:tt)*) => {};
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"select!".to_string()),
        "expected select! in {:?}",
        names
    );
}

#[test]
fn macro_rules_without_macro_export_not_indexed() {
    let source = r#"
macro_rules! internal {
    () => {};
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"internal!".to_string()),
        "internal macro should not be indexed"
    );
}

#[test]
fn macro_export_with_multiple_preceding_attrs() {
    let source = r#"
#[doc(hidden)]
#[macro_export]
macro_rules! join {
    () => {};
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"join!".to_string()),
        "expected join! when #[macro_export] is not the first attr: {:?}",
        names
    );
}

#[test]
fn proc_macro_derive_indexes_derive_name() {
    let source = r#"
#[proc_macro_derive(Serialize)]
pub fn derive_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Serialize".to_string()),
        "expected Serialize in {:?}",
        names
    );
}

#[test]
fn proc_macro_derive_with_attributes_arg_indexes_derive_name_only() {
    let source = r#"
#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn derive_deserialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parse(source);
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
    let source = r#"
#[proc_macro_attribute]
pub fn route(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"route".to_string()),
        "expected route in {:?}",
        names
    );
}

#[test]
fn proc_macro_function_indexes_function_name() {
    let source = r#"
#[proc_macro]
pub fn my_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}
"#;
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"my_macro".to_string()),
        "expected my_macro in {:?}",
        names
    );
}
