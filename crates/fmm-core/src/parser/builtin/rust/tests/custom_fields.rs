use super::support::parse;

#[test]
fn rust_custom_fields_unsafe() {
    let source = "fn foo() { unsafe { std::ptr::null() }; }\nfn bar() { unsafe { 1 }; }";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("unsafe_blocks").unwrap().as_u64().unwrap(), 2);
}

#[test]
fn rust_custom_fields_derives() {
    let source = "#[derive(Debug, Clone, Serialize)]\npub struct Foo {}";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Debug"));
    assert!(names.contains(&"Clone"));
    assert!(names.contains(&"Serialize"));
}

#[test]
fn rust_no_custom_fields_when_clean() {
    let source = "pub struct Empty {}";
    let result = parse(source);
    assert!(result.custom_fields.is_none());
}

#[test]
fn rust_custom_fields_trait_impls() {
    let source = "struct Foo {}\nimpl Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n}\nimpl Clone for Foo {\n    fn clone(&self) -> Self { Foo {} }\n}";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Clone for Foo"));
    assert!(names.contains(&"Display for Foo"));
}

#[test]
fn rust_custom_fields_lifetimes() {
    let source = "pub struct Ref<'a> {\n    data: &'a str,\n}";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
    let names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"'a"));
}

#[test]
fn rust_custom_fields_async_functions() {
    let source = "async fn fetch() {}\nasync fn process() {}\nfn sync_fn() {}";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("async_functions").unwrap().as_u64().unwrap(), 2);
}

#[test]
fn rust_scoped_trait_impl() {
    let source = "struct Foo {}\nimpl std::fmt::Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }\n}";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Display for Foo"));
}

#[test]
fn rust_anonymous_lifetime_filtered() {
    let source = "fn foo(x: &'_ str) {}";
    let result = parse(source);
    if let Some(fields) = result.custom_fields
        && let Some(lts) = fields.get("lifetimes")
    {
        let names: Vec<&str> = lts
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(!names.contains(&"'_"));
    }
}
