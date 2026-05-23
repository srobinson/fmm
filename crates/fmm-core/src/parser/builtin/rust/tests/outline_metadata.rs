use super::support::{get_method, parse};
use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};

#[test]
fn rust_declarations_carry_outline_metadata() {
    let source = r#"
pub mod api {
    pub fn public_fn() {}
}

mod hidden {
    pub fn hidden_public() {}
}

pub(crate) const LIMIT: usize = 10;
pub(super) type Alias = usize;
pub(in crate::api) trait Worker {}

#[test]
fn exercises_test_kind() {}

pub struct PublicType {
    pub(crate) internal_field: String,
}

impl PublicType {
    pub fn new(internal_field: String) -> Self {
        Self { internal_field }
    }
}

pub enum Mode {
    Fast,
}

#[macro_export]
macro_rules! exported_macro {
    () => {};
}
"#;
    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_entry(
        exports,
        "api",
        SymbolVisibility::Public,
        DeclarationKind::Module,
    );
    assert_entry(
        exports,
        "public_fn",
        SymbolVisibility::Public,
        DeclarationKind::Fn,
    );
    assert_entry(
        exports,
        "hidden_public",
        SymbolVisibility::NonExported,
        DeclarationKind::Fn,
    );
    assert_entry(
        exports,
        "LIMIT",
        SymbolVisibility::Crate,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "Alias",
        SymbolVisibility::Crate,
        DeclarationKind::Type,
    );
    assert_entry(
        exports,
        "Worker",
        SymbolVisibility::Crate,
        DeclarationKind::Trait,
    );
    assert_entry(
        exports,
        "exercises_test_kind",
        SymbolVisibility::Private,
        DeclarationKind::Test,
    );
    assert_entry(
        exports,
        "PublicType",
        SymbolVisibility::Public,
        DeclarationKind::Struct,
    );
    assert_child_entry(
        exports,
        "PublicType",
        "internal_field",
        SymbolVisibility::Crate,
        DeclarationKind::Field,
    );
    assert_entry(
        exports,
        "impl PublicType",
        SymbolVisibility::Private,
        DeclarationKind::Impl,
    );
    assert_child_entry(
        exports,
        "PublicType",
        "new",
        SymbolVisibility::Public,
        DeclarationKind::Method,
    );
    assert_entry(
        exports,
        "Mode",
        SymbolVisibility::Public,
        DeclarationKind::Enum,
    );
    assert_child_entry(
        exports,
        "Mode",
        "Fast",
        SymbolVisibility::Private,
        DeclarationKind::Variant,
    );
    let macro_entry = assert_entry(
        exports,
        "exported_macro!",
        SymbolVisibility::Public,
        DeclarationKind::Macro,
    );
    assert_eq!(
        macro_entry.signature.as_deref(),
        Some("macro_rules! exported_macro")
    );
}

#[test]
fn rust_signature_is_declaration_header_text() {
    let source = r#"
/// doc text is outside the declaration node
pub fn public_api(input: usize) -> usize {
    input + 1
}

pub struct PublicType {
    pub(crate) internal_field: String,
}

pub mod api {
    pub fn nested() {}
}
"#;
    let result = parse(source);
    let public_api = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "public_api")
        .expect("public_api should be indexed");
    assert_eq!(
        public_api.signature.as_deref(),
        Some("pub fn public_api(input: usize) -> usize")
    );

    let field = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "internal_field")
        .expect("field should be indexed");
    assert_eq!(
        field.signature.as_deref(),
        Some("pub(crate) internal_field: String")
    );

    let public_type = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "PublicType")
        .expect("PublicType should be indexed");
    assert_eq!(
        public_type.signature.as_deref(),
        Some("pub struct PublicType")
    );

    let module = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == "api")
        .expect("api module should be indexed");
    assert_eq!(module.signature.as_deref(), Some("pub mod api"));
}

#[test]
fn rust_impl_method_private_visibility_is_indexed() {
    let source = "pub struct Foo;\nimpl Foo {\n    fn internal() {}\n}";
    let result = parse(source);
    let entry =
        get_method(&result.metadata.exports, "Foo", "internal").expect("method should be indexed");
    assert_eq!(entry.visibility, Some(SymbolVisibility::Private));
    assert_eq!(entry.declaration_kind, Some(DeclarationKind::Method));
}

#[test]
fn rust_duplicate_nested_names_are_not_collapsed() {
    let source = r#"
pub mod first {
    pub fn same() {}
}

pub mod second {
    pub fn same() {}
}
"#;
    let result = parse(source);
    let matches = result
        .metadata
        .exports
        .iter()
        .filter(|entry| entry.name == "same")
        .count();
    assert_eq!(
        matches, 2,
        "same-named nested declarations should both index"
    );
}

#[test]
fn rust_duplicate_impl_blocks_are_not_collapsed() {
    let source = r#"
pub struct Parser;

impl Parser {
    pub fn new() -> Self {
        Self
    }
}

impl Parser {
    pub fn parse(&self) {}
}
"#;
    let result = parse(source);
    let matches = result
        .metadata
        .exports
        .iter()
        .filter(|entry| entry.name == "impl Parser")
        .count();
    assert_eq!(matches, 2, "same-type impl blocks should both index");
}

fn assert_entry<'a>(
    exports: &'a [ExportEntry],
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) -> &'a ExportEntry {
    let entry = exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.is_none())
        .unwrap_or_else(|| panic!("{name} should be indexed; exports: {exports:?}"));
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(entry.signature.is_some(), "{name} should carry signature");
    entry
}

fn assert_child_entry(
    exports: &[ExportEntry],
    parent: &str,
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) {
    let entry = exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.as_deref() == Some(parent))
        .unwrap_or_else(|| panic!("{parent}.{name} should be indexed; exports: {exports:?}"));
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(
        entry.signature.is_some(),
        "{parent}.{name} should carry signature"
    );
}
