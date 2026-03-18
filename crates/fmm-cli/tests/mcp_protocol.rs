//! Subprocess MCP protocol tests via assert_cmd.
//!
//! Spawns the compiled `fmm serve` binary and exercises the full JSON-RPC
//! loop over stdio: initialize, tools/list, tools/call, unknown method.
//! Tests generate a SQLite index in a temp dir so the server has real data.

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

/// Create a temp dir with source files and a generated `.fmm.db`.
fn setup_fixture() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/app.ts",
        "export function createApp() {\n  return {};\n}\n\nexport const APP_VERSION = '1.0';\n",
    );
    write_file(
        root,
        "src/utils.ts",
        "export function formatDate(d: Date): string {\n  return d.toISOString();\n}\n",
    );

    // Run `fmm generate` to build the index
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .arg("generate")
        .current_dir(root)
        .output()
        .expect("failed to run fmm generate");
    assert!(
        output.status.success(),
        "fmm generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    tmp
}

/// Send JSON-RPC lines to a spawned `fmm serve` process and collect responses.
fn run_mcp_session(root: &std::path::Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::cargo_bin("fmm")
        .unwrap()
        .arg("serve")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn fmm serve");

    // Write all requests then close stdin
    {
        let stdin = child.stdin.as_mut().expect("stdin not piped");
        for req in requests {
            writeln!(stdin, "{}", serde_json::to_string(req).unwrap()).unwrap();
        }
    }
    // Close stdin by dropping it (implicit when child.stdin is taken)
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("failed to wait on fmm serve");

    // Parse each line of stdout as a JSON-RPC response
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).expect("invalid JSON in response"))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn mcp_protocol_initialize() {
    let fixture = setup_fixture();
    let responses = run_mcp_session(
        fixture.path(),
        &[json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1"}
            }
        })],
    );

    assert_eq!(responses.len(), 1, "expected 1 response");
    let resp = &responses[0];
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["error"].is_null(), "initialize must not return error");

    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert!(
        result["capabilities"].is_object(),
        "capabilities must be present"
    );
    assert!(
        result["serverInfo"]["name"].is_string(),
        "serverInfo.name must be present"
    );
}

#[test]
fn mcp_protocol_tools_list() {
    let fixture = setup_fixture();
    let responses = run_mcp_session(
        fixture.path(),
        &[json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        })],
    );

    assert_eq!(responses.len(), 1);
    let tools = responses[0]["result"]["tools"]
        .as_array()
        .expect("tools must be an array");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    let expected = [
        "fmm_lookup_export",
        "fmm_list_exports",
        "fmm_dependency_graph",
        "fmm_search",
        "fmm_read_symbol",
        "fmm_file_outline",
        "fmm_list_files",
        "fmm_glossary",
    ];
    for name in &expected {
        assert!(
            tool_names.contains(name),
            "tools/list missing {name}; got: {tool_names:?}"
        );
    }
}

#[test]
fn mcp_protocol_tools_call() {
    let fixture = setup_fixture();
    let responses = run_mcp_session(
        fixture.path(),
        &[json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "fmm_list_files",
                "arguments": {}
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert!(resp["error"].is_null(), "tools/call must not error");

    let content = resp["result"]["content"]
        .as_array()
        .expect("content must be array");
    assert!(!content.is_empty(), "content must not be empty");

    let text = content[0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("src/app.ts"),
        "list_files must include src/app.ts; got:\n{text}"
    );
}

#[test]
fn mcp_protocol_unknown_method() {
    let fixture = setup_fixture();
    let responses = run_mcp_session(
        fixture.path(),
        &[json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "nonexistent/method"
        })],
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert!(
        resp["result"].is_null(),
        "unknown method must have no result"
    );
    assert_eq!(
        resp["error"]["code"], -32601,
        "must return method not found"
    );
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("nonexistent/method"),
        "error must name the method"
    );
}

#[test]
fn mcp_protocol_full_session() {
    // Full multi-turn session: initialize -> tools/list -> tools/call -> error
    let fixture = setup_fixture();
    let responses = run_mcp_session(
        fixture.path(),
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "test", "version": "0.1"}
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "fmm_lookup_export",
                    "arguments": {"name": "createApp"}
                }
            }),
        ],
    );

    // notifications/initialized is a notification (no id), so no response
    assert_eq!(
        responses.len(),
        3,
        "3 responses (initialize + tools/list + tools/call); notifications produce none"
    );

    // initialize
    assert_eq!(responses[0]["id"], 1);
    assert!(responses[0]["result"]["protocolVersion"].is_string());

    // tools/list
    assert_eq!(responses[1]["id"], 2);
    assert!(responses[1]["result"]["tools"].is_array());

    // tools/call for lookup_export
    assert_eq!(responses[2]["id"], 3);
    let text = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        text.contains("createApp") && text.contains("src/app.ts"),
        "lookup_export must resolve createApp to src/app.ts; got:\n{text}"
    );
}
