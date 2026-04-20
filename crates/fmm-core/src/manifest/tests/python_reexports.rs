use super::super::Manifest;
use super::support::entry;
use crate::parser::Metadata;
use std::collections::HashMap;

#[test]
fn python_reexport_does_not_claim_export_index_slot() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 3,
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("bar", 2, 2)],
            imports: vec![],
            dependencies: vec!["./foo".to_string()],
            loc: 3,
            named_imports: named,
            ..Default::default()
        },
    );

    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/foo.py".to_string()),
        "re-export must not shadow the original definition"
    );
    let all = manifest.export_all.get("bar").unwrap();
    assert_eq!(all.len(), 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"pkg/foo.py"));
    assert!(files.contains(&"pkg/__init__.py"));
}

#[test]
fn python_aliased_reexport_treated_as_local_bind() {
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

    assert_eq!(
        manifest.export_index.get("baz"),
        Some(&"pkg/__init__.py".to_string())
    );
    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/foo.py".to_string())
    );
}

#[test]
fn python_true_name_collision_tracked_in_export_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("bar", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("bar", 1, 5)],
            ..Default::default()
        },
    );

    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/b.py".to_string())
    );
    let all = manifest.export_all.get("bar").unwrap();
    assert!(all.len() >= 2, "shadowed names must track all definitions");
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"pkg/a.py"));
    assert!(files.contains(&"pkg/b.py"));
}
