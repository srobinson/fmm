use super::super::Manifest;
use super::support::entry;
use crate::parser::Metadata;

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

    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"src/config/defaults.rs".to_string())
    );
    let all = manifest.export_all.get("Config").unwrap();
    assert_eq!(all.len(), 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"src/config/types.rs"));
    assert!(files.contains(&"src/config/defaults.rs"));
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

    manifest.add_file("src/app.js", meta_js);
    manifest.add_file("src/app.ts", meta_ts);

    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
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
