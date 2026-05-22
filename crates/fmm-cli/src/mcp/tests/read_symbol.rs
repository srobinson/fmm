use super::support::{assert_error, test_server, test_server_at, tool_text};
use fmm_core::manifest::Manifest;
use fmm_core::parser::{DeclarationKind, ExportEntry, Metadata, SymbolVisibility};
use serde_json::json;

#[test]
fn read_symbol_dotted_notation_returns_method_source() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("factory.ts");
    std::fs::write(
        &file_path,
        "class NestFactoryStatic {\n  create() {\n    return 1;\n  }\n}\n",
    )
    .unwrap();

    let mut manifest = Manifest::new();
    manifest.add_file(
        "factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 5),
                ExportEntry::method("create".to_string(), 2, 4, "NestFactoryStatic".to_string()),
            ],
            loc: 5,
            ..Default::default()
        },
    );

    let server = test_server_at(manifest, dir.path().to_path_buf());
    let text = tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "NestFactoryStatic.create"}),
    );

    assert!(!text.starts_with("ERROR:"), "expected success, got: {text}");
    assert!(text.contains("create"), "should contain method body");
    assert!(
        text.contains("factory.ts"),
        "should contain file name, got: {text}"
    );

    let text = tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "NestFactoryStatic"}),
    );
    assert!(
        !text.starts_with("ERROR:"),
        "class lookup should succeed, got: {text}",
    );
}

#[test]
fn read_symbol_dotted_notation_reports_indexed_kind_and_omits_private_absence() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("service.ts");
    std::fs::write(
        &file_path,
        "class Service {\n  config = \"x\";\n  start() {\n    return this.config;\n  }\n  private helper() {\n    return 1;\n  }\n}\n",
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
                    ..ExportEntry::new("Service".to_string(), 1, 9)
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
            loc: 9,
            ..Default::default()
        },
    );

    let server = test_server_at(manifest, dir.path().to_path_buf());

    let field_text = tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "Service.config"}),
    );
    assert!(
        field_text.contains("kind: field"),
        "field kind missing, got: {field_text}"
    );
    assert!(
        field_text.contains("config = \"x\""),
        "field source missing, got: {field_text}"
    );

    let method_text = tool_text(&server, "fmm_read_symbol", json!({"name": "Service.start"}));
    assert!(
        method_text.contains("kind: method"),
        "method kind missing, got: {method_text}"
    );

    let private_text = tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "Service.helper"}),
    );
    assert!(
        !private_text.starts_with("ERROR:"),
        "private resolver should still work, got: {private_text}"
    );
    assert!(
        !private_text.contains("kind:"),
        "private resolver has no index metadata and should omit kind, got: {private_text}"
    );
}

#[test]
fn read_symbol_dotted_not_found_gives_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(
        &server,
        "fmm_read_symbol",
        json!({"name": "MyClass.missingMethod"}),
    );

    assert_error(&text);
    assert!(
        text.contains("fmm_file_outline"),
        "should suggest fmm_file_outline, got: {text}",
    );
}

#[test]
fn read_symbol_follows_reexport_to_concrete_definition() {
    let dir = tempfile::tempdir().unwrap();
    let init_path = dir.path().join("agno").join("__init__.py");
    let agent_path = dir.path().join("agno").join("agent").join("agent.py");
    std::fs::create_dir_all(agent_path.parent().unwrap()).unwrap();

    std::fs::write(
        &init_path,
        "from .agent.agent import Agent\n__all__ = ['Agent']\n",
    )
    .unwrap();

    let agent_src =
        "class Agent:\n    def __init__(self):\n        pass\n    def run(self):\n        pass\n";
    std::fs::write(&agent_path, agent_src).unwrap();

    let mut manifest = Manifest::new();
    manifest.add_file(
        "agno/__init__.py",
        Metadata {
            exports: vec![ExportEntry::new("Agent".to_string(), 1, 1)],
            imports: vec!["agno.agent.agent".to_string()],
            loc: 2,
            ..Default::default()
        },
    );
    manifest.add_file(
        "agno/agent/agent.py",
        Metadata {
            exports: vec![ExportEntry::new("Agent".to_string(), 1, 5)],
            loc: 5,
            ..Default::default()
        },
    );

    let server = test_server_at(manifest, dir.path().to_path_buf());
    let text = tool_text(&server, "fmm_read_symbol", json!({"name": "Agent"}));

    assert!(
        text.contains("agno/agent/agent.py"),
        "should resolve to concrete definition, got: {text}",
    );
    assert!(
        !text.contains("__init__.py"),
        "should not use reexport site, got: {text}",
    );
    assert!(
        text.contains("class Agent"),
        "should include class body, got: {text}",
    );
}
