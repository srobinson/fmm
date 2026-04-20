use super::support::parse;

#[test]
fn class_public_method_indexed() {
    let source = "export class Foo {\n  public bar(): void {}\n}\n";
    let result = parse(source);
    let method = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "bar")
        .unwrap();
    assert_eq!(method.parent_class.as_deref(), Some("Foo"));
    assert_eq!(method.start_line, 2);
    assert!(!result.metadata.export_names().contains(&"bar".to_string()));
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
}

#[test]
fn class_private_method_not_indexed() {
    let source = "export class Foo {\n  private baz(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "baz"));
}

#[test]
fn class_protected_method_not_indexed() {
    let source = "export class Foo {\n  protected qux(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "qux"));
}

#[test]
fn class_constructor_indexed() {
    let source = "export class Foo {\n  constructor(x: number) {}\n}\n";
    let result = parse(source);
    let ctor = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "constructor");
    assert!(ctor.is_some(), "constructor should be indexed");
    assert_eq!(ctor.unwrap().parent_class.as_deref(), Some("Foo"));
}

#[test]
fn class_no_modifier_is_public() {
    let source = "export class Foo {\n  doThing(): void {}\n}\n";
    let result = parse(source);
    assert!(result.metadata.exports.iter().any(|e| e.name == "doThing"));
}

#[test]
fn non_exported_class_methods_not_indexed() {
    let source = "class Internal {\n  run(): void {}\n}\n";
    let result = parse(source);
    assert!(!result.metadata.exports.iter().any(|e| e.name == "run"));
}

#[test]
fn class_method_line_range_correct() {
    let source = "export class Svc {\n  create() {\n    return 1;\n  }\n  destroy() {}\n}\n";
    let result = parse(source);
    let create = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "create")
        .unwrap();
    assert_eq!(create.start_line, 2);
    assert_eq!(create.end_line, 4);

    let destroy = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "destroy")
        .unwrap();
    assert_eq!(destroy.start_line, 5);
}
