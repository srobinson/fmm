use super::support::{get_method, parse};

#[test]
fn python_methods_public_included() {
    let source = "class Foo:\n    def bar(self):\n        pass\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "Foo", "bar").is_some(),
        "Foo.bar should be indexed"
    );
}

#[test]
fn python_methods_private_excluded() {
    let source = "class Foo:\n    def _internal(self):\n        pass\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "Foo", "_internal").is_none(),
        "Foo._internal should not be indexed"
    );
}

#[test]
fn python_methods_init_included() {
    let source = "class Foo:\n    def __init__(self):\n        pass\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "Foo", "__init__").is_some(),
        "Foo.__init__ should be indexed"
    );
}

#[test]
fn python_methods_other_dunder_excluded() {
    let source = "class Foo:\n    def __str__(self):\n        return ''\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "Foo", "__str__").is_none(),
        "Foo.__str__ should not be indexed"
    );
}

#[test]
fn python_methods_of_underscore_class_are_indexed() {
    let source = "class _Internal:\n    def method(self):\n        pass\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "_Internal", "method").is_some(),
        "methods of underscore prefixed classes should be indexed"
    );
}

#[test]
fn python_methods_decorated_included() {
    let source = "class Foo:\n    @property\n    def value(self):\n        return self._value\n    @staticmethod\n    def create():\n        return Foo()\n";
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "Foo", "value").is_some(),
        "Foo.value should be indexed"
    );
    assert!(
        get_method(&result.metadata.exports, "Foo", "create").is_some(),
        "Foo.create should be indexed"
    );
}

#[test]
fn python_methods_decorated_line_range_includes_decorator() {
    let source = "class Foo:\n    @property\n    def value(self):\n        return 1\n";
    let result = parse(source);
    let entry =
        get_method(&result.metadata.exports, "Foo", "value").expect("Foo.value should be indexed");

    assert_eq!(entry.start_line, 2);
}

#[test]
fn python_methods_dunder_all_respects_export_list() {
    let source = r#"
__all__ = ["PublicClass"]

class PublicClass:
    def method(self):
        pass

class HiddenClass:
    def method(self):
        pass
"#;
    let result = parse(source);

    assert!(
        get_method(&result.metadata.exports, "PublicClass", "method").is_some(),
        "PublicClass.method should be indexed"
    );
    assert!(
        get_method(&result.metadata.exports, "HiddenClass", "method").is_none(),
        "HiddenClass.method should not be indexed"
    );
}
