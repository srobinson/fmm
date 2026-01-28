use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use crate::manifest::Manifest;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
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
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

pub struct McpServer {
    manifest: Option<Manifest>,
}

impl McpServer {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_default();
        let manifest = Manifest::load(&root).ok().flatten();
        Self { manifest }
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

            let response = self.handle_request(&request);

            if let Some(resp) = response {
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
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
        let tools = vec![
            Tool {
                name: "fmm_find_export".to_string(),
                description: "Find which file exports a given symbol. Returns the file path.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The export name to find"
                        }
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "fmm_list_exports".to_string(),
                description: "List all exports from a specific file.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "The file path to list exports from"
                        }
                    },
                    "required": ["file"]
                }),
            },
            Tool {
                name: "fmm_search".to_string(),
                description: "Search the codebase manifest. Find files by exports, imports, or line count.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "export": {
                            "type": "string",
                            "description": "Find file that exports this symbol"
                        },
                        "imports": {
                            "type": "string",
                            "description": "Find files that import this module"
                        },
                        "min_loc": {
                            "type": "integer",
                            "description": "Minimum lines of code"
                        },
                        "max_loc": {
                            "type": "integer",
                            "description": "Maximum lines of code"
                        }
                    }
                }),
            },
            Tool {
                name: "fmm_get_manifest".to_string(),
                description: "Get the full manifest with all file metadata. Use for understanding project structure.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "fmm_file_info".to_string(),
                description: "Get metadata for a specific file (exports, imports, dependencies, loc).".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "The file path to get info for"
                        }
                    },
                    "required": ["file"]
                }),
            },
        ];

        Ok(json!({ "tools": tools }))
    }

    fn handle_tool_call(&self, params: &Option<Value>) -> Result<Value, JsonRpcError> {
        let params = params.as_ref().ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing params".to_string(),
            data: None,
        })?;

        let tool_name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing tool name".to_string(),
            data: None,
        })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name {
            "fmm_find_export" => self.tool_find_export(&arguments),
            "fmm_list_exports" => self.tool_list_exports(&arguments),
            "fmm_search" => self.tool_search(&arguments),
            "fmm_get_manifest" => self.tool_get_manifest(),
            "fmm_file_info" => self.tool_file_info(&arguments),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(text) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            })),
            Err(e) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": e
                }],
                "isError": true
            })),
        }
    }

    fn tool_find_export(&self, args: &Value) -> Result<String, String> {
        let manifest = self.manifest.as_ref().ok_or("No manifest found. Run 'fmm generate' first.")?;

        let name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing 'name' argument")?;

        match manifest.export_index.get(name) {
            Some(file) => Ok(file.clone()),
            None => Err(format!("Export '{}' not found", name)),
        }
    }

    fn tool_list_exports(&self, args: &Value) -> Result<String, String> {
        let manifest = self.manifest.as_ref().ok_or("No manifest found. Run 'fmm generate' first.")?;

        let file = args.get("file").and_then(|v| v.as_str()).ok_or("Missing 'file' argument")?;

        match manifest.files.get(file) {
            Some(entry) => {
                if entry.exports.is_empty() {
                    Ok("No exports".to_string())
                } else {
                    Ok(entry.exports.join(", "))
                }
            }
            None => Err(format!("File '{}' not found in manifest", file)),
        }
    }

    fn tool_search(&self, args: &Value) -> Result<String, String> {
        let manifest = self.manifest.as_ref().ok_or("No manifest found. Run 'fmm generate' first.")?;

        let mut results: Vec<(&String, &crate::manifest::FileEntry)> = Vec::new();

        // Search by export
        if let Some(export) = args.get("export").and_then(|v| v.as_str()) {
            if let Some(file_path) = manifest.export_index.get(export) {
                if let Some(entry) = manifest.files.get(file_path) {
                    results.push((file_path, entry));
                }
            }
        }

        // Search by imports
        if let Some(import_name) = args.get("imports").and_then(|v| v.as_str()) {
            for (file_path, entry) in &manifest.files {
                if entry.imports.iter().any(|i| i.contains(import_name)) {
                    if !results.iter().any(|(f, _)| *f == file_path) {
                        results.push((file_path, entry));
                    }
                }
            }
        }

        // Filter by LOC
        let min_loc = args.get("min_loc").and_then(|v| v.as_u64()).map(|v| v as usize);
        let max_loc = args.get("max_loc").and_then(|v| v.as_u64()).map(|v| v as usize);

        if min_loc.is_some() || max_loc.is_some() {
            // If no other filters, search all files
            if results.is_empty() && args.get("export").is_none() && args.get("imports").is_none() {
                for (file_path, entry) in &manifest.files {
                    results.push((file_path, entry));
                }
            }

            results.retain(|(_, entry)| {
                let passes_min = min_loc.map_or(true, |min| entry.loc >= min);
                let passes_max = max_loc.map_or(true, |max| entry.loc <= max);
                passes_min && passes_max
            });
        }

        // If no filters, return all
        if args.get("export").is_none()
            && args.get("imports").is_none()
            && min_loc.is_none()
            && max_loc.is_none()
        {
            for (file_path, entry) in &manifest.files {
                results.push((file_path, entry));
            }
        }

        if results.is_empty() {
            Ok("No matches found".to_string())
        } else {
            let output: Vec<String> = results
                .iter()
                .map(|(path, entry)| {
                    format!(
                        "{} (exports: {}, loc: {})",
                        path,
                        entry.exports.join(", "),
                        entry.loc
                    )
                })
                .collect();
            Ok(output.join("\n"))
        }
    }

    fn tool_get_manifest(&self) -> Result<String, String> {
        let manifest = self.manifest.as_ref().ok_or("No manifest found. Run 'fmm generate' first.")?;

        serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())
    }

    fn tool_file_info(&self, args: &Value) -> Result<String, String> {
        let manifest = self.manifest.as_ref().ok_or("No manifest found. Run 'fmm generate' first.")?;

        let file = args.get("file").and_then(|v| v.as_str()).ok_or("Missing 'file' argument")?;

        match manifest.files.get(file) {
            Some(entry) => {
                Ok(format!(
                    "File: {}\nExports: {}\nImports: {}\nDependencies: {}\nLOC: {}",
                    file,
                    if entry.exports.is_empty() { "none".to_string() } else { entry.exports.join(", ") },
                    if entry.imports.is_empty() { "none".to_string() } else { entry.imports.join(", ") },
                    if entry.dependencies.is_empty() { "none".to_string() } else { entry.dependencies.join(", ") },
                    entry.loc
                ))
            }
            None => Err(format!("File '{}' not found in manifest", file)),
        }
    }
}
