use super::support::parse;

#[test]
fn function_names_populated() {
    let source = "pub fn foo() {}\npub fn bar() {}\npub struct Baz {}\n";
    let result = parse(source);
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
    let source = "pub fn exported() {}\nfn private_helper() {}\n";
    let result = parse(source);
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
    let source = "pub struct Foo {}\npub enum Bar { A, B }\n";
    let result = parse(source);
    let has_fn = result
        .custom_fields
        .as_ref()
        .and_then(|cf| cf.get("function_names"))
        .is_some();
    assert!(!has_fn, "no functions should mean no function_names key");
}
