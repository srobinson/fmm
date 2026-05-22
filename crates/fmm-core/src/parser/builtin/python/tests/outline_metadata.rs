use super::support::parse;
use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};

#[test]
fn python_exports_carry_outline_metadata() {
    let source = r#"
from abc import ABC
from typing import Final, Protocol, TypeAlias

PUBLIC_LIMIT: Final[int] = 10
WidgetId: TypeAlias = str

def load_widget(name: str) -> str:
    return name

def test_widget_loads():
    assert load_widget("a") == "a"

@pytest.fixture
def widget_fixture():
    return "a"

class Store(Protocol):
    pass

class Worker(ABC):
    pass

class Widget:
    pass

def _helper():
    pass
"#;

    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_entry(
        exports,
        "PUBLIC_LIMIT",
        SymbolVisibility::Public,
        DeclarationKind::Const,
    );
    assert_entry(
        exports,
        "WidgetId",
        SymbolVisibility::Public,
        DeclarationKind::Type,
    );
    assert_entry(
        exports,
        "load_widget",
        SymbolVisibility::Public,
        DeclarationKind::Fn,
    );
    assert_entry(
        exports,
        "test_widget_loads",
        SymbolVisibility::Public,
        DeclarationKind::Test,
    );
    assert_entry(
        exports,
        "widget_fixture",
        SymbolVisibility::Public,
        DeclarationKind::Test,
    );
    assert_entry(
        exports,
        "Store",
        SymbolVisibility::Public,
        DeclarationKind::Trait,
    );
    assert_entry(
        exports,
        "Worker",
        SymbolVisibility::Public,
        DeclarationKind::Trait,
    );
    assert_entry(
        exports,
        "Widget",
        SymbolVisibility::Public,
        DeclarationKind::Struct,
    );
    assert_entry(
        exports,
        "_helper",
        SymbolVisibility::Private,
        DeclarationKind::Fn,
    );
}

#[test]
fn python_class_members_carry_outline_metadata() {
    let source = r#"
class Widget:
    label: str = "default"
    _token = "secret"

    def render(self) -> str:
        return self.label
"#;

    let result = parse(source);
    let exports = &result.metadata.exports;

    assert_child_entry(
        exports,
        "Widget",
        "label",
        SymbolVisibility::Public,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Widget",
        "_token",
        SymbolVisibility::Private,
        DeclarationKind::Field,
    );
    assert_child_entry(
        exports,
        "Widget",
        "render",
        SymbolVisibility::Public,
        DeclarationKind::Method,
    );
}

#[test]
fn python_signature_is_declaration_header_text() {
    let source = r#"
def load_widget(name: str) -> str:
    """body docstring is not part of the signature"""
    return name

class Widget:
    """body docstring is not part of the signature"""
    label: str = "default"

    def render(self) -> str:
        return self.label
"#;

    let result = parse(source);
    let load_widget = find_entry(&result.metadata.exports, "load_widget");
    assert_eq!(
        load_widget.signature.as_deref(),
        Some("def load_widget(name: str) -> str")
    );

    let widget = find_entry(&result.metadata.exports, "Widget");
    assert_eq!(widget.signature.as_deref(), Some("class Widget"));

    let render = find_child_entry(&result.metadata.exports, "Widget", "render");
    assert_eq!(render.signature.as_deref(), Some("def render(self) -> str"));
}

fn assert_entry(
    exports: &[ExportEntry],
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) {
    let entry = find_entry(exports, name);
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(entry.signature.is_some(), "{name} should carry a signature");
}

fn assert_child_entry(
    exports: &[ExportEntry],
    parent: &str,
    name: &str,
    visibility: SymbolVisibility,
    kind: DeclarationKind,
) {
    let entry = find_child_entry(exports, parent, name);
    assert_eq!(entry.visibility, Some(visibility));
    assert_eq!(entry.declaration_kind, Some(kind));
    assert!(
        entry.signature.is_some(),
        "{parent}.{name} should carry a signature"
    );
}

fn find_entry<'a>(exports: &'a [ExportEntry], name: &str) -> &'a ExportEntry {
    exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.is_none())
        .unwrap_or_else(|| panic!("{name} should be indexed; exports: {exports:?}"))
}

fn find_child_entry<'a>(exports: &'a [ExportEntry], parent: &str, name: &str) -> &'a ExportEntry {
    exports
        .iter()
        .find(|entry| entry.name == name && entry.parent_class.as_deref() == Some(parent))
        .unwrap_or_else(|| panic!("{parent}.{name} should be indexed; exports: {exports:?}"))
}
