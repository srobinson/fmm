use super::support::parse;

#[test]
fn parse_python_dunder_all() {
    let source = r#"
__all__ = ["public_func", "PublicClass"]

def public_func():
    pass

def _private_func():
    pass

class PublicClass:
    pass

class _InternalClass:
    pass
"#;
    let result = parse(source);

    assert_eq!(
        result.metadata.export_names(),
        vec!["public_func", "PublicClass"]
    );

    let exports = &result.metadata.exports;
    let func_export = exports.iter().find(|e| e.name == "public_func").unwrap();
    assert_eq!(func_export.start_line, 4);
    assert_eq!(func_export.end_line, 5);

    let class_export = exports.iter().find(|e| e.name == "PublicClass").unwrap();
    assert_eq!(class_export.start_line, 10);
    assert_eq!(class_export.end_line, 11);
}

#[test]
fn dunder_all_reexport_from_relative_import_range_matches_import_line() {
    let source = r#"
from .foo import bar

__all__ = ["bar"]
"#;
    let result = parse(source);
    let bar = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "bar")
        .expect("bar should be exported");

    assert_eq!(bar.start_line, 2);
    assert_eq!(bar.end_line, 2);
}

#[test]
fn dunder_all_reexport_aliased_uses_alias_as_key() {
    let source = r#"
from .foo import bar as baz

__all__ = ["baz"]
"#;
    let result = parse(source);
    let baz = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "baz")
        .expect("baz should be exported");

    assert_eq!(baz.start_line, 2);
    assert_eq!(baz.end_line, 2);
}

#[test]
fn dunder_all_reexport_from_plain_import_range_matches_import_line() {
    let source = r#"
import mymod

__all__ = ["mymod"]
"#;
    let result = parse(source);
    let m = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "mymod")
        .expect("mymod should be exported");

    assert_eq!(m.start_line, 2);
    assert_eq!(m.end_line, 2);
}

#[test]
fn dunder_all_mixed_local_and_reexport() {
    let source = r#"
from .helpers import shared

def local_fn():
    return 1

__all__ = ["shared", "local_fn"]
"#;
    let result = parse(source);
    let shared = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "shared")
        .expect("shared should be exported");
    let local = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "local_fn")
        .expect("local_fn should be exported");

    assert_eq!(shared.start_line, 2);
    assert_eq!(local.start_line, 4);
    assert_eq!(local.end_line, 5);
}

#[test]
fn dunder_all_reexport_aliased_plain_import() {
    let source = r#"
import mymod as other

__all__ = ["other"]
"#;
    let result = parse(source);
    let other = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "other")
        .expect("other should be exported");

    assert_eq!(other.start_line, 2);
    assert_eq!(other.end_line, 2);
}

#[test]
fn dunder_all_reexport_dotted_plain_import_uses_first_segment() {
    let source = r#"
import os.path

__all__ = ["os"]
"#;
    let result = parse(source);
    let os = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "os")
        .expect("os should be exported");

    assert_eq!(os.start_line, 2);
    assert_eq!(os.end_line, 2);
}

#[test]
fn dunder_all_unknown_name_falls_back_to_zero() {
    let source = r#"
__all__ = ["orphan"]

def actual_thing():
    pass
"#;
    let result = parse(source);
    let orphan = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "orphan")
        .expect("orphan should be in exports even without a definition");

    assert_eq!(orphan.start_line, 0);
    assert_eq!(orphan.end_line, 0);
}

#[test]
fn dunder_all_multiline_import_uses_full_statement_range() {
    let source = r#"
from .pkg import (
    one,
    two,
)

__all__ = ["one", "two"]
"#;
    let result = parse(source);
    let one = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "one")
        .unwrap();
    let two = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "two")
        .unwrap();

    assert_eq!(one.start_line, 2);
    assert_eq!(one.end_line, 5);
    assert_eq!(two.start_line, 2);
    assert_eq!(two.end_line, 5);
}

#[test]
fn parse_python_dunder_all_with_decorated_class() {
    let source = r#"
from dataclasses import dataclass

__all__ = ["DecoratedModel", "bare_func"]

@dataclass
class DecoratedModel:
    id: int
    name: str

def bare_func():
    pass
"#;
    let result = parse(source);

    assert_eq!(
        result.metadata.export_names(),
        vec!["DecoratedModel", "bare_func"]
    );

    let model = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "DecoratedModel")
        .unwrap();
    assert_eq!(model.start_line, 6);
    assert_eq!(model.end_line, 9);
}

#[test]
fn parse_python_dunder_all_overrides_discovery() {
    let source = r#"
__all__ = ["only_this"]

def only_this():
    pass

def also_public():
    pass
"#;
    let result = parse(source);

    assert_eq!(result.metadata.export_names(), vec!["only_this"]);
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"also_public".to_string())
    );
}
