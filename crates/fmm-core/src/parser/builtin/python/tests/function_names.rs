use super::support::parse;

fn function_names(source: &str) -> Vec<String> {
    let result = parse(source);
    let fields = result.custom_fields.expect("custom_fields should be Some");

    fields
        .get("function_names")
        .expect("function_names key")
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn function_names_populated() {
    let source = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass\n";
    let names = function_names(source);

    assert!(names.contains(&"foo".to_string()), "foo missing: {names:?}");
    assert!(names.contains(&"bar".to_string()), "bar missing: {names:?}");
    assert!(
        !names.contains(&"Baz".to_string()),
        "class should not be in function_names: {names:?}"
    );
}

#[test]
fn function_names_excludes_private() {
    let source = "def public():\n    pass\n\ndef _private():\n    pass\n";
    let names = function_names(source);

    assert!(
        names.contains(&"public".to_string()),
        "public missing: {names:?}"
    );
    assert!(
        !names.contains(&"_private".to_string()),
        "_private should be excluded: {names:?}"
    );
}

#[test]
fn function_names_empty_for_no_functions() {
    let source = "class Foo:\n    pass\n\nBAR = 42\n";
    let result = parse(source);
    let has_fn = result
        .custom_fields
        .as_ref()
        .and_then(|cf| cf.get("function_names"))
        .is_some();

    assert!(!has_fn, "no functions should mean no function_names key");
}
