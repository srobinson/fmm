use super::support::parse;

#[test]
fn parse_python_functions() {
    let source = "def hello():\n    pass\n\ndef world():\n    pass\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"world".to_string())
    );
    assert_eq!(result.metadata.loc, 5);
}

#[test]
fn parse_python_classes() {
    let source = "class MyClass:\n    pass\n\nclass _Private:\n    pass\n";
    let result = parse(source);
    let names = result.metadata.export_names();

    assert!(names.contains(&"MyClass".to_string()));
    assert!(names.contains(&"_Private".to_string()));
}

#[test]
fn parse_python_underscore_functions_are_surfaced() {
    let source = "def _private():\n    pass\n\ndef public():\n    pass\n";
    let result = parse(source);
    let names = result.metadata.export_names();

    assert!(names.contains(&"_private".to_string()));
    assert!(names.contains(&"public".to_string()));
}
