use super::support::parse;

#[test]
fn parse_rust_pub_functions() {
    let source = "pub fn hello() {}\nfn private() {}\npub fn world() {}";
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
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"private".to_string())
    );
}

#[test]
fn parse_rust_pub_structs_and_enums() {
    let source = "pub struct Foo {}\npub enum Bar { A, B }\nstruct Private {}";
    let result = parse(source);
    assert!(result.metadata.export_names().contains(&"Foo".to_string()));
    assert!(result.metadata.export_names().contains(&"Bar".to_string()));
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"Private".to_string())
    );
}

#[test]
fn parse_rust_pub_crate_excluded() {
    let source = "pub fn visible() {}\npub(crate) fn internal() {}\npub(super) fn parent_only() {}";
    let result = parse(source);
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"visible".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"internal".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"parent_only".to_string())
    );
}

#[test]
fn parse_rust_loc() {
    let source = "pub fn hello() {\n    42\n}\n";
    let result = parse(source);
    assert_eq!(result.metadata.loc, 3);
}
