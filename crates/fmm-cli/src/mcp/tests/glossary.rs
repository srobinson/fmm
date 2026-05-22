use super::support::{test_server, test_server_at, tool_text};
use fmm_core::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};
use fmm_core::parser::{DeclarationKind, ExportEntry, Metadata, SymbolVisibility};
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

#[test]
fn glossary_dotted_field_reports_kind_without_method_call_implication() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("service.ts"),
        "export class Service {\n  config = \"x\";\n  start() {\n    return this.config;\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("caller.ts"),
        "import { Service } from './service';\nfunction config() { return 0; }\nconst svc = new Service();\nsvc.config;\nconfig();\n",
    )
    .unwrap();

    let mut manifest = Manifest::new();
    manifest.add_file(
        "service.ts",
        Metadata {
            exports: vec![
                ExportEntry {
                    declaration_kind: Some(DeclarationKind::Struct),
                    visibility: Some(SymbolVisibility::Public),
                    ..ExportEntry::new("Service".to_string(), 1, 6)
                },
                ExportEntry {
                    name: "config".to_string(),
                    start_line: 2,
                    end_line: 2,
                    signature: Some("config = \"x\"".to_string()),
                    visibility: Some(SymbolVisibility::Public),
                    declaration_kind: Some(DeclarationKind::Field),
                    parent_class: Some("Service".to_string()),
                    relationship_kind: None,
                },
                ExportEntry {
                    declaration_kind: Some(DeclarationKind::Method),
                    visibility: Some(SymbolVisibility::Public),
                    ..ExportEntry::method("start".to_string(), 3, 5, "Service".to_string())
                },
            ],
            loc: 6,
            ..Default::default()
        },
    );
    manifest.add_file(
        "caller.ts",
        Metadata {
            imports: vec!["./service".to_string()],
            dependencies: vec!["service.ts".to_string()],
            loc: 5,
            ..Default::default()
        },
    );

    let server = test_server_at(manifest, dir.path().to_path_buf());
    let field_text = tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "Service.config"}),
    );

    assert!(
        field_text.contains("kind: field"),
        "field kind missing, got:\n{field_text}"
    );
    assert!(
        field_text.contains("(field; no external source callers expected)"),
        "field-specific empty message missing, got:\n{field_text}"
    );
    assert!(
        field_text.contains("field access is not a method call site"),
        "field access explanation missing, got:\n{field_text}"
    );
    assert!(
        !field_text.contains("none call config directly"),
        "field output must not imply a method call, got:\n{field_text}"
    );

    let method_text = tool_text(&server, "fmm_glossary", json!({"pattern": "Service.start"}));
    assert!(
        method_text.contains("kind: method"),
        "method kind missing, got:\n{method_text}"
    );
    assert!(
        method_text.contains("none call start directly"),
        "method no-caller wording should remain, got:\n{method_text}"
    );
}
