use super::super::Manifest;
use super::support::entry;
use crate::parser::Metadata;
use std::collections::HashMap;

#[test]
fn reexports_in_file_resolves_to_origin() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("bar", 2, 2)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "bar");
    assert_eq!(rx[0].origin_file, "pkg/foo.py");
    assert_eq!(rx[0].origin_start, 1);
    assert_eq!(rx[0].origin_end, 3);
}

#[test]
fn reexports_in_file_mixed_local_and_reexports() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 10, 20)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("main", 5, 15), entry("bar", 3, 3)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "bar");
    assert_eq!(rx[0].origin_file, "pkg/foo.py");
    assert_eq!(rx[0].origin_start, 10);
}

#[test]
fn reexports_in_file_ignores_aliased_imports() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("baz", 2, 2)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert!(
        rx.is_empty(),
        "aliased import should not be treated as a re-export; got: {:?}",
        rx
    );
}

#[test]
fn reexports_in_file_only_local_defs_returns_empty() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/mod.py",
        Metadata {
            exports: vec![entry("foo", 1, 10), entry("bar", 12, 20)],
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/mod.py");
    assert!(rx.is_empty());
}

#[test]
fn reexports_in_file_falls_back_to_import_line_when_origin_missing() {
    let mut manifest = Manifest::new();
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert("sys".to_string(), vec!["sys_exit".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("sys_exit", 4, 4)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "sys_exit");
    assert_eq!(
        rx[0].origin_file, "pkg/__init__.py",
        "fallback should point at the re-exporter itself"
    );
    assert_eq!(rx[0].origin_start, 4, "fallback uses the import line");
    assert_eq!(rx[0].origin_end, 4);
}

#[test]
fn reexports_in_file_sorted_alphabetically() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("zeta", 1, 2)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("alpha", 1, 2)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".a".to_string(), vec!["zeta".to_string()]);
    named.insert(".b".to_string(), vec!["alpha".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("zeta", 2, 2), entry("alpha", 3, 3)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    let names: Vec<&str> = rx.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "zeta"]);
}

#[test]
fn reexports_in_file_unknown_file_returns_empty() {
    let manifest = Manifest::new();
    let rx = manifest.reexports_in_file("does/not/exist.py");
    assert!(rx.is_empty());
}
