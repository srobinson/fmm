use super::support::{assert_error, test_server, tool_text};
use fmm_core::manifest::Manifest;
use fmm_core::parser::{ExportEntry, Metadata};
use serde_json::json;

#[test]
fn lookup_export_dotted_name_resolves_via_method_index() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 200),
                ExportEntry::method(
                    "createApplicationContext".to_string(),
                    166,
                    195,
                    "NestFactoryStatic".to_string(),
                ),
            ],
            loc: 200,
            ..Default::default()
        },
    );
    let server = test_server(manifest);

    let text = tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "NestFactoryStatic.createApplicationContext"}),
    );

    assert!(!text.starts_with("ERROR:"), "expected success, got: {text}");
    assert!(text.contains("src/factory.ts"), "got: {text}");
    assert!(text.contains("166"), "got: {text}");
    assert!(text.contains("195"), "got: {text}");
}

#[test]
fn lookup_export_flat_name_still_works_after_method_index_added() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![ExportEntry::new("NestFactoryStatic".to_string(), 1, 200)],
            loc: 200,
            ..Default::default()
        },
    );
    let server = test_server(manifest);

    let text = tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "NestFactoryStatic"}),
    );

    assert!(
        !text.starts_with("ERROR:"),
        "flat lookup should succeed, got: {text}"
    );
    assert!(text.contains("src/factory.ts"), "got: {text}");
}

#[test]
fn lookup_export_unknown_dotted_name_returns_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(
        &server,
        "fmm_lookup_export",
        json!({"name": "MyClass.ghostMethod"}),
    );

    assert_error(&text);
}
