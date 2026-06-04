use anyhow::Result;
use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

mod args;
mod schema;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
pub(crate) mod tools;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Cap MCP tool responses to prevent context bombs.
/// Large responses get truncated to disk by Claude, defeating the purpose.
pub(crate) const MAX_RESPONSE_BYTES: usize = 10_240;

type StoreLoader<S> = fn(&Path) -> Result<S, String>;

fn cap_response(text: String, truncate: bool, accepts_truncate: bool) -> String {
    if !truncate || text.len() <= MAX_RESPONSE_BYTES {
        return text;
    }
    // Find a valid UTF-8 boundary at or before MAX_RESPONSE_BYTES
    let byte_limit = MAX_RESPONSE_BYTES;
    let safe_limit = text.floor_char_boundary(byte_limit);
    let truncated = &text[..safe_limit];
    // Find last newline to avoid cutting mid-line
    let cut_point = truncated.rfind('\n').unwrap_or(safe_limit);
    let mut result = text[..cut_point].to_string();
    let total_lines = text.lines().count();
    let shown_lines = result.lines().count();
    let hint = if accepts_truncate {
        " Use truncate: false to get the full response."
    } else {
        ""
    };
    result.push_str(&format!(
        "\n\n[Truncated — showing {}/{} lines.{}]",
        shown_lines, total_lines, hint
    ));
    result
}

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

/// MCP server generic over a store backend.
///
/// The `store` field is `Option<S>` to support two construction paths:
/// - Production: `from_store(store, root)` provides a concrete store for live reload.
/// - Testing: direct struct construction with `store: None` and an injected manifest.
///
/// When `store` is `None`, `reload()` tries `store_loader` if one was supplied.
/// Tests can omit the loader and use an injected manifest.
pub struct McpServer<S: FmmStore> {
    pub(crate) store: Option<S>,
    pub(crate) manifest: Option<Manifest>,
    pub(crate) load_error: Option<String>,
    pub(crate) root: PathBuf,
    pub(crate) store_loader: Option<StoreLoader<S>>,
}

/// Type alias for the SQLite-backed MCP server (production default).
pub type SqliteMcpServer = McpServer<fmm_store::SqliteStore>;

// --- SqliteStore-specific constructors (backward-compatible API) ---

impl Default for McpServer<fmm_store::SqliteStore> {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer<fmm_store::SqliteStore> {
    /// Create an MCP server rooted at the current working directory.
    ///
    /// Opens (or gracefully handles the absence of) a SQLite store at `$CWD/.fmm.db`.
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_default();
        Self::with_root(root)
    }

    /// Create an MCP server rooted at `root`.
    ///
    /// Attempts to open an existing SQLite store. If the database does not exist,
    /// the server starts without a store and reports the error on tool calls.
    pub fn with_root(root: PathBuf) -> Self {
        match open_sqlite_store(&root) {
            Ok(store) => {
                let mut server = Self::from_store(store, root);
                server.store_loader = Some(open_sqlite_store);
                server
            }
            Err(e) => Self {
                store: None,
                manifest: None,
                load_error: Some(e),
                root,
                store_loader: Some(open_sqlite_store),
            },
        }
    }
}

fn open_sqlite_store(root: &Path) -> Result<fmm_store::SqliteStore, String> {
    fmm_store::SqliteStore::open(root).map_err(|e| e.to_string())
}

// --- Generic implementation (works with any FmmStore backend) ---

impl<S: FmmStore> McpServer<S> {
    /// Create an MCP server from an opened store.
    ///
    /// Eagerly loads the manifest; if the load fails, the error is captured
    /// and reported when tools are called.
    pub fn from_store(store: S, root: PathBuf) -> Self {
        let (manifest, load_error) = match store.load_manifest() {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self {
            store: Some(store),
            manifest,
            load_error,
            root,
            store_loader: None,
        }
    }

    fn reload(&mut self) {
        if self.store.is_none()
            && let Some(store_loader) = self.store_loader
        {
            match store_loader(&self.root) {
                Ok(store) => {
                    self.store = Some(store);
                }
                Err(e) => {
                    self.manifest = None;
                    self.load_error = Some(e);
                    return;
                }
            }
        }

        if let Some(store) = &self.store {
            match store.load_manifest() {
                Ok(m) => {
                    self.manifest = Some(m);
                    self.load_error = None;
                }
                Err(e) => {
                    self.manifest = None;
                    self.load_error = Some(e.to_string());
                }
            }
        }
    }

    /// Call a tool by name with JSON arguments. Useful for testing.
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, JsonRpcError> {
        let params = json!({"name": name, "arguments": arguments});
        self.handle_tool_call(&Some(params))
    }

    fn require_manifest(&self) -> Result<&Manifest, String> {
        self.manifest.as_ref().ok_or_else(|| {
            self.load_error
                .clone()
                .unwrap_or_else(|| "No index found. Run 'fmm generate' first.".to_string())
        })
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

            // Reload index before handling tool calls
            if request.method == "tools/call" {
                self.reload();
            }

            let response = self.handle_request(&request);

            if let Some(resp) = response {
                // WORKAROUND: handle BrokenPipe gracefully (cascade vector V4 from
                // anthropics/claude-code#22264 -- Claude Code may close the pipe when
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
            "fmm_dependency_cycles" => {
                tools::tool_dependency_cycles(manifest, &self.root, &arguments)
            }
            "fmm_search" => tools::tool_search(manifest, &self.root, &arguments),
            "fmm_read_symbol" => tools::tool_read_symbol(manifest, &self.root, &arguments),
            "fmm_file_outline" => tools::tool_file_outline(manifest, &self.root, &arguments),
            "fmm_list_files" => tools::tool_list_files(manifest, &self.root, &arguments),
            "fmm_glossary" => tools::tool_glossary(manifest, &self.root, &arguments),
            "fmm_find_similar" => tools::tool_find_similar(manifest, &self.root, &arguments),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        let accepts_truncate = matches!(
            tool_name,
            "fmm_file_outline" | "fmm_read_symbol" | "fmm_glossary"
        );
        let should_truncate = if accepts_truncate {
            arguments
                .get("truncate")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        } else {
            true
        };

        match result {
            Ok(text) => {
                let text = cap_response(text, should_truncate, accepts_truncate);
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
