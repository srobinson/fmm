use super::support::{get_method, parse};

#[test]
fn rust_impl_methods_get_own_line_ranges() {
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
    let result = parse(source);
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

    assert!(exports[0].start_line <= exports[1].start_line);
    assert!(exports[1].start_line <= exports[2].start_line);
}

#[test]
fn rust_impl_pub_fn_indexed_as_method() {
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn bar() {}\n}";
    let result = parse(source);
    let entry =
        get_method(&result.metadata.exports, "Foo", "bar").expect("Foo.bar should be indexed");
    assert_eq!(entry.parent_class.as_deref(), Some("Foo"));
}

#[test]
fn rust_impl_private_fn_not_indexed() {
    let source = "pub struct Foo;\nimpl Foo {\n    fn internal() {}\n}";
    let result = parse(source);
    assert!(
        get_method(&result.metadata.exports, "Foo", "internal").is_none(),
        "Foo.internal (no pub) should NOT be indexed"
    );
}

#[test]
fn rust_trait_impl_pub_fn_indexed() {
    let source = "pub struct Foo;\ntrait MyTrait {\n    fn method(&self);\n}\nimpl MyTrait for Foo {\n    pub fn method(&self) {}\n}";
    let result = parse(source);
    assert!(
        get_method(&result.metadata.exports, "Foo", "method").is_some(),
        "Foo.method from trait impl should be indexed"
    );
}

#[test]
fn rust_impl_non_exported_struct_not_indexed() {
    let source = "struct Hidden;\nimpl Hidden {\n    pub fn method() {}\n}";
    let result = parse(source);
    assert!(
        get_method(&result.metadata.exports, "Hidden", "method").is_none(),
        "methods of non-exported struct should NOT be indexed"
    );
}

#[test]
fn rust_impl_method_line_range_spans_full_body() {
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn bar() {\n        42\n    }\n}";
    let result = parse(source);
    let entry =
        get_method(&result.metadata.exports, "Foo", "bar").expect("Foo.bar should be indexed");
    assert_eq!(entry.start_line, 3);
    assert_eq!(entry.end_line, 5);
}

#[test]
fn rust_impl_generic_type_indexed() {
    let source = "pub struct Wrapper<T>(T);\nimpl<T> Wrapper<T> {\n    pub fn inner(&self) -> &T { &self.0 }\n}";
    let result = parse(source);
    assert!(
        get_method(&result.metadata.exports, "Wrapper", "inner").is_some(),
        "Wrapper<T>.inner should be indexed under Wrapper"
    );
}

#[test]
fn rust_impl_methods_have_correct_parent_class() {
    let source = "pub struct Foo;\nimpl Foo {\n    pub fn new() -> Self { Foo }\n    pub fn get_x(&self) -> i32 { 0 }\n}";
    let result = parse(source);
    let exports = &result.metadata.exports;

    let new_fn = get_method(exports, "Foo", "new").expect("Foo.new should be indexed");
    assert_eq!(new_fn.parent_class.as_deref(), Some("Foo"));

    let get_x = get_method(exports, "Foo", "get_x").expect("Foo.get_x should be indexed");
    assert_eq!(get_x.parent_class.as_deref(), Some("Foo"));

    let foo = exports
        .iter()
        .find(|e| e.name == "Foo" && e.parent_class.is_none())
        .expect("Foo should be a top-level export");
    assert_eq!(foo.start_line, 1);
}
