use super::support::{assert_error, test_server, test_server_at, tool_text};
use fmm_core::manifest::Manifest;
use fmm_core::parser::{ExportEntry, Metadata};
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
