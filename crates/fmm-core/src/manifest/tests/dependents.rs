use super::super::Manifest;
use super::support::entry;
use crate::parser::Metadata;

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
