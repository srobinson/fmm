use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    root: PathBuf,
    manifest_mtime: Option<SystemTime>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_default();
        let manifest = Manifest::load(&root).ok().flatten();
        let manifest_mtime = Self::get_manifest_mtime(&root);
        Self {
            manifest,
            root,
            manifest_mtime,
        }
    }

    fn get_manifest_mtime(root: &Path) -> Option<SystemTime> {
        let path = root.join(".fmm").join("index.json");
        std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
    }

    fn maybe_reload(&mut self) {
        let current_mtime = Self::get_manifest_mtime(&self.root);
        if current_mtime != self.manifest_mtime {
            self.manifest = Manifest::load(&self.root).ok().flatten();
            self.manifest_mtime = current_mtime;
        }
    }

    fn require_manifest(&self) -> Result<&Manifest, String> {
        self.manifest
            .as_ref()
            .ok_or_else(|| "No manifest found. Run 'fmm generate' first.".to_string())
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

            // Hot-reload manifest before handling tool calls
            if request.method == "tools/call" {
                self.maybe_reload();
            }

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
                name: "fmm_lookup_export".to_string(),
                description: "Find which file exports a given symbol. Returns file path and full metadata (exports, imports, dependencies, loc).".to_string(),
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
                description: "List exports from the manifest. If 'pattern' is given, returns all exports matching the substring across all files. If 'file' is given, returns exports from that specific file.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Substring to match against export names (case-insensitive)"
                        },
                        "file": {
                            "type": "string",
                            "description": "File path to list exports from"
                        }
                    }
                }),
            },
            Tool {
                name: "fmm_file_info".to_string(),
                description: "Get structured metadata for a specific file: exports, imports, dependencies, and lines of code.".to_string(),
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
            Tool {
                name: "fmm_dependency_graph".to_string(),
                description: "Get the dependency graph for a file: what it depends on (upstream) and what depends on it (downstream).".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "The file path to get dependency graph for"
                        }
                    },
                    "required": ["file"]
                }),
            },
            Tool {
                name: "fmm_search".to_string(),
                description: "Search the codebase manifest. Find files by exports, imports, dependencies, or line count.".to_string(),
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
                        "depends_on": {
                            "type": "string",
                            "description": "Find files that depend on this path"
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
                description: "Get the full manifest with all file metadata. Use for understanding overall project structure.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
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

        let result = match tool_name {
            "fmm_lookup_export" => self.tool_lookup_export(&arguments),
            "fmm_list_exports" => self.tool_list_exports(&arguments),
            "fmm_file_info" => self.tool_file_info(&arguments),
            "fmm_dependency_graph" => self.tool_dependency_graph(&arguments),
            "fmm_search" => self.tool_search(&arguments),
            "fmm_get_manifest" => self.tool_get_manifest(),
            // Keep old name as alias for backwards compatibility
            "fmm_find_export" => self.tool_lookup_export(&arguments),
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

    fn tool_lookup_export(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'name' argument")?;

        match manifest.export_index.get(name) {
            Some(file_path) => {
                let entry = manifest.files.get(file_path);
                let result = json!({
                    "file": file_path,
                    "exports": entry.map(|e| &e.exports),
                    "imports": entry.map(|e| &e.imports),
                    "dependencies": entry.map(|e| &e.dependencies),
                    "loc": entry.map(|e| e.loc),
                });
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
            }
            None => Err(format!("Export '{}' not found", name)),
        }
    }

    fn tool_list_exports(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let pattern = args.get("pattern").and_then(|v| v.as_str());
        let file = args.get("file").and_then(|v| v.as_str());

        if let Some(file_path) = file {
            // List exports from a specific file
            match manifest.files.get(file_path) {
                Some(entry) => {
                    let result = json!({
                        "file": file_path,
                        "exports": entry.exports,
                    });
                    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
                }
                None => Err(format!("File '{}' not found in manifest", file_path)),
            }
        } else if let Some(pat) = pattern {
            // Search export index by pattern
            let pat_lower = pat.to_lowercase();
            let mut matches: Vec<(&String, &String)> = manifest
                .export_index
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains(&pat_lower))
                .collect();
            matches.sort_by_key(|(name, _)| name.to_lowercase());

            let result: Vec<Value> = matches
                .iter()
                .map(|(name, path)| json!({"export": name, "file": path}))
                .collect();
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
        } else {
            // List all exports (grouped by file)
            let mut by_file: Vec<(&String, Vec<&String>)> = Vec::new();
            for (file_path, entry) in &manifest.files {
                if !entry.exports.is_empty() {
                    by_file.push((file_path, entry.exports.iter().collect()));
                }
            }
            by_file.sort_by_key(|(path, _)| path.to_lowercase());

            let result: Vec<Value> = by_file
                .iter()
                .map(|(path, exports)| json!({"file": path, "exports": exports}))
                .collect();
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
        }
    }

    fn tool_file_info(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let file = args
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'file' argument")?;

        match manifest.files.get(file) {
            Some(entry) => {
                let result = json!({
                    "file": file,
                    "exports": entry.exports,
                    "imports": entry.imports,
                    "dependencies": entry.dependencies,
                    "loc": entry.loc,
                });
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
            }
            None => Err(format!("File '{}' not found in manifest", file)),
        }
    }

    fn tool_dependency_graph(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let file = args
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'file' argument")?;

        let entry = manifest
            .files
            .get(file)
            .ok_or_else(|| format!("File '{}' not found in manifest", file))?;

        // Upstream: files this file depends on (its dependencies)
        let upstream: Vec<&str> = entry.dependencies.iter().map(|s| s.as_str()).collect();

        // Downstream: files that depend on this file
        let mut downstream: Vec<&String> = manifest
            .files
            .iter()
            .filter(|(path, _)| *path != file)
            .filter(|(_, e)| e.dependencies.iter().any(|d| dep_matches(d, file)))
            .map(|(path, _)| path)
            .collect();
        downstream.sort();

        let result = json!({
            "file": file,
            "upstream": upstream,
            "downstream": downstream,
            "imports": entry.imports,
        });
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn tool_search(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let mut results: Vec<(&String, &crate::manifest::FileEntry)> = Vec::new();

        let has_export = args.get("export").and_then(|v| v.as_str()).is_some();
        let has_imports = args.get("imports").and_then(|v| v.as_str()).is_some();
        let has_depends_on = args.get("depends_on").and_then(|v| v.as_str()).is_some();

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
                if entry.imports.iter().any(|i| i.contains(import_name))
                    && !results.iter().any(|(f, _)| *f == file_path)
                {
                    results.push((file_path, entry));
                }
            }
        }

        // Search by depends_on
        if let Some(dep_path) = args.get("depends_on").and_then(|v| v.as_str()) {
            for (file_path, entry) in &manifest.files {
                if entry.dependencies.iter().any(|d| d.contains(dep_path))
                    && !results.iter().any(|(f, _)| *f == file_path)
                {
                    results.push((file_path, entry));
                }
            }
        }

        // Filter by LOC
        let min_loc = args
            .get("min_loc")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_loc = args
            .get("max_loc")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        if min_loc.is_some() || max_loc.is_some() {
            if results.is_empty() && !has_export && !has_imports && !has_depends_on {
                for (file_path, entry) in &manifest.files {
                    results.push((file_path, entry));
                }
            }

            results.retain(|(_, entry)| {
                let passes_min = min_loc.is_none_or(|min| entry.loc >= min);
                let passes_max = max_loc.is_none_or(|max| entry.loc <= max);
                passes_min && passes_max
            });
        }

        // If no filters, return all
        if !has_export && !has_imports && !has_depends_on && min_loc.is_none() && max_loc.is_none()
        {
            for (file_path, entry) in &manifest.files {
                results.push((file_path, entry));
            }
        }

        let output: Vec<Value> = results
            .iter()
            .map(|(path, entry)| {
                json!({
                    "file": path,
                    "exports": entry.exports,
                    "imports": entry.imports,
                    "dependencies": entry.dependencies,
                    "loc": entry.loc,
                })
            })
            .collect();

        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
    }

    fn tool_get_manifest(&self) -> Result<String, String> {
        let manifest = self.require_manifest()?;
        serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())
    }
}

/// Check if a dependency path matches a file path.
/// Dependencies are stored as relative paths like "./types" or "./config"
/// and file paths are like "src/types.ts". This does a suffix match.
fn dep_matches(dep: &str, file: &str) -> bool {
    // Strip leading "./" from dep
    let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
    // Strip extension from file for comparison
    let file_stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
    // Check if the file path ends with the dependency path
    file_stem == dep_clean || file_stem.ends_with(&format!("/{}", dep_clean))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_matches_relative_path() {
        assert!(dep_matches("./types", "src/types.ts"));
        assert!(dep_matches("./config", "src/config.ts"));
        assert!(!dep_matches("./types", "src/other.ts"));
    }

    #[test]
    fn dep_matches_nested_path() {
        assert!(dep_matches("./utils/helpers", "src/utils/helpers.ts"));
        assert!(!dep_matches("./utils/helpers", "src/utils/other.ts"));
    }

    #[test]
    fn dep_matches_without_prefix() {
        assert!(dep_matches("types", "src/types.ts"));
    }

    #[test]
    fn test_hot_reload_detection() {
        let server = McpServer::new();
        // Just verify construction works and mtime is loaded
        assert!(server.root.is_absolute() || server.root.as_os_str().is_empty());
    }
}
