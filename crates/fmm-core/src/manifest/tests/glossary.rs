use super::super::{GlossaryMode, Manifest};
use super::support::entry;
use crate::parser::Metadata;

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
