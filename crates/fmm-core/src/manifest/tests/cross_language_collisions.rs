use super::super::Manifest;
use super::support::entry;
use crate::parser::Metadata;

#[test]
fn same_language_python_collision_last_wins_and_tracked_in_export_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("UsageStats"),
        Some(&"pkg/b.py".to_string()),
        "last writer must win the single-pick slot"
    );
    let all = manifest.export_all.get("UsageStats").unwrap();
    assert!(all.len() >= 2, "shadowed names must track all definitions");
}

#[test]
fn cross_language_python_ts_collision_tracked_in_export_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "api/a.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "web/b.ts",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("UsageStats"),
        Some(&"web/b.ts".to_string())
    );
    let all = manifest.export_all.get("UsageStats").unwrap();
    assert!(all.len() >= 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"api/a.py"));
    assert!(files.contains(&"web/b.ts"));
}

#[test]
fn cross_language_rust_ts_collision_tracked_in_export_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "crates/core/src/a.rs",
        Metadata {
            exports: vec![entry("Config", 1, 10)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "web/b.ts",
        Metadata {
            exports: vec![entry("Config", 1, 10)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"web/b.ts".to_string())
    );
    let all = manifest.export_all.get("Config").unwrap();
    assert!(all.len() >= 2);
}
