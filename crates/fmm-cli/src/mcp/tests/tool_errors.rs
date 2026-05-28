use super::support::{assert_error, test_server, tool_text};
use fmm_core::manifest::Manifest;
use serde_json::json;

#[test]
fn dependency_graph_directory_path_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_dependency_graph", json!({"file": "src/mcp/"}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {text}",
    );
}

#[test]
fn read_symbol_empty_name_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_read_symbol", json!({"name": ""}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_exports"),
        "should suggest fmm_list_exports, got: {text}",
    );
}

#[test]
fn file_outline_directory_path_returns_helpful_error() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_file_outline", json!({"file": "src/cli/"}));

    assert_error(&text);
    assert!(
        text.contains("fmm_list_files"),
        "should suggest fmm_list_files, got: {text}",
    );
}

#[test]
fn search_unknown_field_returns_invalid_arguments() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_search", json!({"symbol": "Logger"}));

    assert_error(&text);
    assert!(text.contains("Invalid arguments"), "got: {text}");
    assert!(text.contains("unknown field `symbol`"), "got: {text}");
}

#[test]
fn list_files_unknown_field_returns_invalid_arguments() {
    let server = test_server(Manifest::new());
    let text = tool_text(&server, "fmm_list_files", json!({"symbol": "Logger"}));

    assert_error(&text);
    assert!(text.contains("Invalid arguments"), "got: {text}");
    assert!(text.contains("unknown field `symbol`"), "got: {text}");
}

/// Contract guard: every property advertised in a tool's published input schema
/// (generated from `tools.toml`) must be an accepted field on its arg struct.
/// With `#[serde(deny_unknown_fields)]`, a schema property missing from the
/// struct would reject a documented, valid client call at runtime. This walks
/// every tool, sends one type-valid value per advertised property, and asserts
/// the call is never rejected for an unknown field — turning future
/// `tools.toml`/struct drift into a test failure instead of a silent break.
#[test]
fn every_advertised_schema_property_is_accepted_by_its_struct() {
    let server = test_server(Manifest::new());
    let schema = super::super::schema::tool_list();
    let tools = schema["tools"].as_array().expect("schema has tools array");
    assert!(!tools.is_empty(), "schema advertises no tools");

    for tool in tools {
        let name = tool["name"].as_str().expect("tool has a name");
        let Some(props) = tool["inputSchema"]["properties"].as_object() else {
            continue; // tool advertises no parameters
        };

        let args = props
            .iter()
            .map(|(prop, spec)| {
                let value = match spec["type"].as_str() {
                    Some("integer") | Some("number") => json!(1),
                    Some("boolean") => json!(true),
                    _ => json!("x"),
                };
                (prop.clone(), value)
            })
            .collect();

        let text = tool_text(&server, name, serde_json::Value::Object(args));
        assert!(
            !text.contains("unknown field"),
            "tool `{name}`: an advertised schema property is missing from its arg struct \
             (deny_unknown_fields rejected a documented parameter): {text}",
        );
    }
}
