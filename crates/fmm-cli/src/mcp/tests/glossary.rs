use super::support::{test_server, tool_text};
use fmm_core::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn glossary_layer2_filters_non_symbol_importers() {
    let mut manifest = Manifest::new();

    manifest.files.insert(
        "source.js".to_string(),
        FileEntry {
            exports: vec!["myFunc".to_string()],
            export_lines: Some(vec![ExportLines { start: 1, end: 5 }]),
            methods: None,
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            modified: None,
            function_names: vec!["myFunc".to_string()],
            named_imports: HashMap::new(),
            namespace_imports: vec![],
            ..Default::default()
        },
    );

    let mut caller_named = HashMap::new();
    caller_named.insert("./source".to_string(), vec!["myFunc".to_string()]);
    manifest.files.insert(
        "caller.js".to_string(),
        FileEntry {
            exports: vec![],
            export_lines: None,
            methods: None,
            imports: vec!["./source".to_string()],
            dependencies: vec!["source.js".to_string()],
            loc: 5,
            modified: None,
            function_names: vec![],
            named_imports: caller_named,
            namespace_imports: vec![],
            ..Default::default()
        },
    );

    let mut bystander_named = HashMap::new();
    bystander_named.insert("./source".to_string(), vec!["otherThing".to_string()]);
    manifest.files.insert(
        "bystander.js".to_string(),
        FileEntry {
            exports: vec![],
            export_lines: None,
            methods: None,
            imports: vec!["./source".to_string()],
            dependencies: vec!["source.js".to_string()],
            loc: 3,
            modified: None,
            function_names: vec![],
            named_imports: bystander_named,
            namespace_imports: vec![],
            ..Default::default()
        },
    );

    manifest
        .export_index
        .insert("myFunc".to_string(), "source.js".to_string());
    let location = ExportLocation {
        file: "source.js".to_string(),
        lines: Some(ExportLines { start: 1, end: 5 }),
    };
    manifest
        .export_locations
        .insert("myFunc".to_string(), location.clone());
    manifest
        .export_all
        .entry("myFunc".to_string())
        .or_default()
        .push(location.clone());
    manifest
        .function_index
        .insert("myFunc".to_string(), location);

    let server = test_server(manifest);
    let text = tool_text(&server, "fmm_glossary", json!({"pattern": "myFunc"}));

    assert!(
        text.contains("caller.js"),
        "caller.js must be in used_by; got:\n{text}"
    );
    assert!(
        !text.contains("bystander.js"),
        "bystander.js must be filtered by named import matching; got:\n{text}",
    );
    assert!(
        text.contains("additional"),
        "disclosure note must mention additional; got:\n{text}",
    );
}
