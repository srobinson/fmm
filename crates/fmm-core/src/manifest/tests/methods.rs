use super::super::Manifest;
use crate::parser::{ExportEntry, Metadata};

#[test]
fn test_add_file_skips_method_entries_in_export_index() {
    let mut manifest = Manifest::new();
    let metadata = Metadata {
        exports: vec![
            ExportEntry::new("MyClass".to_string(), 1, 50),
            ExportEntry::method("run".to_string(), 5, 20, "MyClass".to_string()),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    manifest.add_file("src/thing.ts", metadata);

    assert!(manifest.export_index.contains_key("MyClass"));
    assert!(!manifest.export_index.contains_key("run"));
    assert!(!manifest.export_index.contains_key("MyClass.run"));
}

#[test]
fn test_method_index_populated_by_add_file() {
    let mut manifest = Manifest::new();
    let metadata = Metadata {
        exports: vec![
            ExportEntry::new("NestFactoryStatic".to_string(), 43, 381),
            ExportEntry::method(
                "create".to_string(),
                55,
                89,
                "NestFactoryStatic".to_string(),
            ),
            ExportEntry::method(
                "createApplicationContext".to_string(),
                132,
                158,
                "NestFactoryStatic".to_string(),
            ),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 400,
        ..Default::default()
    };
    manifest.add_file("src/factory.ts", metadata);

    let loc = manifest
        .method_index
        .get("NestFactoryStatic.createApplicationContext")
        .unwrap();
    assert_eq!(loc.file, "src/factory.ts");
    assert_eq!(loc.lines.as_ref().unwrap().start, 132);
    assert_eq!(loc.lines.as_ref().unwrap().end, 158);

    let create = manifest
        .method_index
        .get("NestFactoryStatic.create")
        .unwrap();
    assert_eq!(create.lines.as_ref().unwrap().start, 55);
    assert!(manifest.export_index.contains_key("NestFactoryStatic"));
}
