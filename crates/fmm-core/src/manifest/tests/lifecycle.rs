use super::super::Manifest;
use super::support::entry;
use crate::identity::FileId;
use crate::parser::{ExportEntry, Metadata};

#[test]
fn test_manifest_add_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("validateUser", 5, 20), entry("createSession", 22, 45)],
        imports: vec!["crypto".to_string()],
        dependencies: vec!["./database".to_string()],
        loc: 234,
        ..Default::default()
    };

    manifest.add_file("src/auth.ts", metadata);

    assert!(manifest.has_file("src/auth.ts"));
    assert_eq!(
        manifest.export_index.get("validateUser"),
        Some(&"src/auth.ts".to_string())
    );
    assert_eq!(
        manifest.export_index.get("createSession"),
        Some(&"src/auth.ts".to_string())
    );
    let loc = manifest.export_locations.get("validateUser").unwrap();
    assert_eq!(loc.lines.as_ref().unwrap().start, 5);
    assert_eq!(loc.lines.as_ref().unwrap().end, 20);
}

#[test]
fn test_manifest_validate_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("test", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata.clone());
    assert!(manifest.validate_file("file.ts", &metadata));

    let different = Metadata {
        exports: vec![entry("different", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    assert!(!manifest.validate_file("file.ts", &different));
}

#[test]
fn validate_file_ignores_method_entries() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![
            entry("MyClass", 1, 20),
            ExportEntry::method("doThing".to_string(), 5, 10, "MyClass".to_string()),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };

    manifest.add_file("svc.ts", metadata.clone());
    assert!(manifest.validate_file("svc.ts", &metadata));
}

#[test]
fn test_manifest_remove_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("toRemove", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 10,
        ..Default::default()
    };

    manifest.add_file("remove.ts", metadata);
    assert!(manifest.has_file("remove.ts"));
    assert!(manifest.export_index.contains_key("toRemove"));

    manifest.remove_file("remove.ts");
    assert!(!manifest.has_file("remove.ts"));
    assert!(!manifest.export_index.contains_key("toRemove"));
    assert!(!manifest.export_locations.contains_key("toRemove"));
}

#[test]
fn test_manifest_update_file_cleans_old_exports() {
    let mut manifest = Manifest::new();

    let metadata1 = Metadata {
        exports: vec![entry("foo", 1, 5), entry("bar", 7, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 10,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata1);

    let metadata2 = Metadata {
        exports: vec![entry("foo", 1, 5), entry("baz", 7, 12)],
        imports: vec![],
        dependencies: vec![],
        loc: 15,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata2);

    assert_eq!(
        manifest.export_index.get("foo"),
        Some(&"file.ts".to_string())
    );
    assert_eq!(
        manifest.export_index.get("baz"),
        Some(&"file.ts".to_string())
    );
    assert!(!manifest.export_index.contains_key("bar"));
    assert_eq!(manifest.file_count(), 1);
}

#[test]
fn manifest_file_identity_survivors_stay_stable_across_incremental_updates() {
    let mut manifest = Manifest::new();
    let metadata = Metadata::default();

    manifest.add_file("src/a.ts", metadata.clone());
    manifest.add_file("src/b.ts", metadata.clone());
    manifest.add_file("src/c.ts", metadata.clone());

    let a_id = manifest.file_id("src/a.ts").unwrap();
    let b_id = manifest.file_id("src/b.ts").unwrap();
    let c_id = manifest.file_id("src/c.ts").unwrap();

    manifest.add_file("src/b.ts", metadata.clone());
    manifest.add_file("src/d.ts", metadata.clone());
    manifest.remove_file("src/b.ts");
    manifest.add_file("src/e.ts", metadata);

    assert_eq!(a_id, FileId(0));
    assert_eq!(b_id, FileId(1));
    assert_eq!(c_id, FileId(2));
    assert_eq!(manifest.file_id("src/a.ts"), Some(a_id));
    assert_eq!(manifest.file_id("src/c.ts"), Some(c_id));
    assert_eq!(manifest.file_id("src/d.ts"), Some(FileId(3)));
    assert_eq!(manifest.file_id("src/e.ts"), Some(FileId(4)));
    assert_eq!(manifest.path_for_file_id(b_id), None);
    assert_eq!(manifest.path_for_file_id(a_id), Some("src/a.ts"));
}
