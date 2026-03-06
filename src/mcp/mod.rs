use crate::manifest::Manifest;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod args;
mod schema;
#[cfg(test)]
mod tests;
pub(crate) mod tools;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub struct McpServer {
    manifest: Option<Manifest>,
    root: PathBuf,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        // Safe default: empty path is harmless; MCP server will report "no sidecars" if cwd fails
        let root = std::env::current_dir().unwrap_or_default();
        Self::with_root(root)
    }

    pub fn with_root(root: PathBuf) -> Self {
        let manifest = Manifest::load_from_sidecars(&root).ok();
        Self { manifest, root }
    }

    fn reload(&mut self) {
        self.manifest = Manifest::load_from_sidecars(&self.root).ok();
    }

    /// Call a tool by name with JSON arguments. Useful for testing.
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, JsonRpcError> {
        let params = json!({"name": name, "arguments": arguments});
        self.handle_tool_call(&Some(params))
    }

    /// Cap MCP tool responses to prevent context bombs.
    /// Large responses get truncated to disk by Claude, defeating the purpose.
    pub(crate) const MAX_RESPONSE_BYTES: usize = 10_240;

    fn cap_response(text: String, truncate: bool) -> String {
        if !truncate || text.len() <= Self::MAX_RESPONSE_BYTES {
            return text;
        }
        // Find a valid UTF-8 boundary at or before MAX_RESPONSE_BYTES
        let byte_limit = Self::MAX_RESPONSE_BYTES;
        let safe_limit = text.floor_char_boundary(byte_limit);
        let truncated = &text[..safe_limit];
        // Find last newline to avoid cutting mid-line
        let cut_point = truncated.rfind('\n').unwrap_or(safe_limit);
        let mut result = text[..cut_point].to_string();
        let total_lines = text.lines().count();
        let shown_lines = result.lines().count();
        result.push_str(&format!(
            "\n\n[Truncated — showing {}/{} lines. Use truncate: false to get the full source.]",
            shown_lines, total_lines
        ));
        result
    }

    fn require_manifest(&self) -> Result<&Manifest, String> {
        self.manifest
            .as_ref()
            .ok_or_else(|| "No sidecars found. Run 'fmm generate' first.".to_string())
    }

    pub fn run(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Value::Null,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                    stdout.flush()?;
                    continue;
                }
            };

            // Rebuild index from sidecars before handling tool calls
            if request.method == "tools/call" {
                self.reload();
            }

            let response = self.handle_request(&request);

            if let Some(resp) = response {
                // WORKAROUND: handle BrokenPipe gracefully (cascade vector V4 from
                // anthropics/claude-code#22264 — Claude Code may close the pipe when
                // it cancels parallel tool calls).
                let write_result = writeln!(stdout, "{}", serde_json::to_string(&resp)?)
                    .and_then(|_| stdout.flush());
                if let Err(e) = write_result {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        return Ok(());
                    }
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    fn handle_request(&mut self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.params),
            "notifications/initialized" => return None,
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tool_call(&request.params),
            "ping" => Ok(json!({})),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        Some(match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(value),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(error),
            },
        })
    }

    fn handle_initialize(&self, _params: &Option<Value>) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "fmm",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        Ok(schema::tool_list())
    }

    fn handle_tool_call(&self, params: &Option<Value>) -> Result<Value, JsonRpcError> {
        let params = params.as_ref().ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let tool_name =
            params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: -32602,
                    message: "Missing tool name".to_string(),
                    data: None,
                })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let manifest = match self.require_manifest() {
            Ok(m) => m,
            Err(e) => {
                return Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("ERROR: {}", e)
                    }]
                }));
            }
        };

        let result = match tool_name {
            // Original tools
            "fmm_lookup_export" => tools::tool_lookup_export(manifest, &self.root, &arguments),
            "fmm_list_exports" => tools::tool_list_exports(manifest, &self.root, &arguments),
            "fmm_dependency_graph" => {
                tools::tool_dependency_graph(manifest, &self.root, &arguments)
            }
            "fmm_search" => tools::tool_search(manifest, &self.root, &arguments),
            "fmm_read_symbol" => tools::tool_read_symbol(manifest, &self.root, &arguments),
            "fmm_file_outline" => tools::tool_file_outline(manifest, &self.root, &arguments),
            "fmm_list_files" => tools::tool_list_files(manifest, &self.root, &arguments),
            "fmm_glossary" => tools::tool_glossary(manifest, &self.root, &arguments),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        // fmm_read_symbol supports truncate: false to bypass the 10KB cap.
        let should_truncate = if tool_name == "fmm_read_symbol" {
            arguments
                .get("truncate")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        } else {
            true
        };

        match result {
            Ok(text) => {
                let text = Self::cap_response(text, should_truncate);
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }))
            }
            // WORKAROUND: Claude Code cancels all sibling parallel MCP tool calls when
            // any tool returns isError:true (Promise.all fail-fast, tracked at
            // anthropics/claude-code#22264). Drop the flag; prefix with ERROR: so the
            // LLM recognises failure from content alone. Revert when #22264 ships
            // Promise.allSettled for MCP tools.
            Err(e) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("ERROR: {}", e)
                }]
            })),
        }
    }
}
