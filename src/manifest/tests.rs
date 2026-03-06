use super::*;
use crate::manifest::GlossaryMode;
use crate::parser::{ExportEntry, Metadata};

fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
    ExportEntry::new(name.to_string(), start, end)
}

#[test]
fn export_all_tracks_all_definitions_including_duplicates() {
    let mut manifest = Manifest::new();

    let meta_a = Metadata {
        exports: vec![entry("Config", 1, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };
    let meta_b = Metadata {
        exports: vec![entry("Config", 5, 15)],
        imports: vec![],
        dependencies: vec![],
        loc: 30,
        ..Default::default()
    };

    manifest.add_file("src/config/types.rs", meta_a);
    manifest.add_file("src/config/defaults.rs", meta_b);

    // export_index last-writer-wins
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"src/config/defaults.rs".to_string())
    );
    // export_all has both
    let all = manifest.export_all.get("Config").unwrap();
    assert_eq!(all.len(), 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"src/config/types.rs"));
    assert!(files.contains(&"src/config/defaults.rs"));
}

#[test]
fn find_dependents_uses_dep_matches() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/config.ts",
        Metadata {
            exports: vec![entry("Config", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/app.ts",
        Metadata {
            exports: vec![entry("App", 1, 10)],
            imports: vec![],
            dependencies: vec!["./config".to_string()],
            loc: 20,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/other.ts",
        Metadata {
            exports: vec![entry("Other", 1, 5)],
            imports: vec![],
            dependencies: vec!["./utils".to_string()],
            loc: 5,
            ..Default::default()
        },
    );

    let deps = manifest.find_dependents("src/config.ts");
    assert_eq!(deps, vec!["src/app.ts"]);
}

#[test]
fn build_glossary_empty_pattern_returns_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/a.ts",
        Metadata {
            exports: vec![entry("Alpha", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    let entries = manifest.build_glossary("", GlossaryMode::All);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
}

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

    // Class is in export_index
    assert!(manifest.export_index.contains_key("MyClass"));
    // Method is NOT in export_index
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

    // Class itself is still in export_index
    assert!(manifest.export_index.contains_key("NestFactoryStatic"));
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
    // Must return true — methods are not top-level exports and should not affect
    // the comparison. Prior to the fix this would return false because method
    // names leaked into current_names.
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
fn cross_file_collision_shadows_old_entry() {
    let mut manifest = Manifest::new();

    let meta_a = Metadata {
        exports: vec![entry("Config", 1, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };
    let meta_b = Metadata {
        exports: vec![entry("Config", 5, 15)],
        imports: vec![],
        dependencies: vec![],
        loc: 30,
        ..Default::default()
    };

    manifest.add_file("src/config/types.rs", meta_a);
    manifest.add_file("src/config/defaults.rs", meta_b);

    // Last writer wins
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"src/config/defaults.rs".to_string())
    );
}

#[test]
fn ts_over_js_priority_no_shadow() {
    let mut manifest = Manifest::new();

    let meta_ts = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    let meta_js = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    manifest.add_file("src/app.ts", meta_ts);
    manifest.add_file("src/app.js", meta_js);

    // .ts should win — .js doesn't overwrite
    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
}

#[test]
fn js_then_ts_order_ts_still_wins() {
    let mut manifest = Manifest::new();

    let meta_js = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    let meta_ts = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    // JS added first, then TS — TS should still win
    manifest.add_file("src/app.js", meta_js);
    manifest.add_file("src/app.ts", meta_ts);

    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
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
fn export_all_remove_file_cleans_up() {
    let mut manifest = Manifest::new();

    manifest.add_file(
        "src/a.ts",
        Metadata {
            exports: vec![entry("Foo", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/b.ts",
        Metadata {
            exports: vec![entry("Foo", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );

    assert_eq!(manifest.export_all.get("Foo").unwrap().len(), 2);

    manifest.remove_file("src/a.ts");
    let remaining = manifest.export_all.get("Foo").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].file, "src/b.ts");
}

#[test]
fn export_all_remove_last_entry_cleans_key() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/only.ts",
        Metadata {
            exports: vec![entry("Unique", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.remove_file("src/only.ts");
    assert!(!manifest.export_all.contains_key("Unique"));
}
