use crate::manifest::Manifest;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

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

// Typed argument structs for MCP tool handlers

#[derive(Debug, Deserialize)]
struct LookupExportArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ListExportsArgs {
    pattern: Option<String>,
    file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileInfoArgs {
    file: String,
}

#[derive(Debug, Deserialize)]
struct DependencyGraphArgs {
    file: String,
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    export: Option<String>,
    imports: Option<String>,
    depends_on: Option<String>,
    min_loc: Option<usize>,
    max_loc: Option<usize>,
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
        let manifest = Manifest::load_from_sidecars(&root).ok();
        Self { manifest, root }
    }

    fn reload(&mut self) {
        self.manifest = Manifest::load_from_sidecars(&self.root).ok();
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
                description: "Instant O(1) symbol-to-file lookup. Find where a function, class, type, or variable is defined. Returns the file path plus metadata (exports, imports, dependencies, LOC). Use before Grep.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Exact export name to find (function, class, type, variable, component)"
                        }
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "fmm_list_exports".to_string(),
                description: "Search or list exported symbols across the codebase. Use 'pattern' for fuzzy discovery (e.g. 'auth' matches validateAuth, authMiddleware). Use 'file' to list a specific file's exports.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Substring to match against export names (case-insensitive). E.g. 'auth' finds all auth-related exports."
                        },
                        "file": {
                            "type": "string",
                            "description": "File path — returns all exports from this specific file"
                        }
                    }
                }),
            },
            Tool {
                name: "fmm_file_info".to_string(),
                description: "Get a file's structural profile from the index: exports, imports, dependencies, LOC. Same data as the file's .fmm sidecar, but from the pre-built index.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "File path to inspect — returns exports, imports, dependencies, LOC without reading source"
                        }
                    },
                    "required": ["file"]
                }),
            },
            Tool {
                name: "fmm_dependency_graph".to_string(),
                description: "Get a file's dependency graph: upstream dependencies (what it imports) and downstream dependents (what would break if it changes). Use for impact analysis and blast radius.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "File path to analyze — returns all upstream dependencies and downstream dependents"
                        }
                    },
                    "required": ["file"]
                }),
            },
            Tool {
                name: "fmm_search".to_string(),
                description: "Search files by structural criteria: exported symbol, imported package, local dependency, or LOC range. Filters combine with AND logic. Use for 'which files use crypto?', 'what depends on auth?'.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "export": {
                            "type": "string",
                            "description": "Find the file that exports this symbol (exact match)"
                        },
                        "imports": {
                            "type": "string",
                            "description": "Find all files that import this package/module (substring match)"
                        },
                        "depends_on": {
                            "type": "string",
                            "description": "Find all files that depend on this local path — use for impact analysis"
                        },
                        "min_loc": {
                            "type": "integer",
                            "description": "Minimum lines of code — find files larger than this"
                        },
                        "max_loc": {
                            "type": "integer",
                            "description": "Maximum lines of code — find files smaller than this"
                        }
                    }
                }),
            },
            // fmm_get_manifest and fmm_project_overview REMOVED —
            // dumping the entire index is an anti-pattern (ALP-396).
            // Use targeted tools: fmm_lookup_export, fmm_search, fmm_dependency_graph.
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
            // Original tools
            "fmm_lookup_export" => self.tool_lookup_export(&arguments),
            "fmm_list_exports" => self.tool_list_exports(&arguments),
            "fmm_file_info" => self.tool_file_info(&arguments),
            "fmm_dependency_graph" => self.tool_dependency_graph(&arguments),
            "fmm_search" => self.tool_search(&arguments),
            // Legacy aliases
            "fmm_find_export" => self.tool_lookup_export(&arguments),
            "fmm_find_symbol" => self.tool_lookup_export(&arguments),
            "fmm_file_metadata" => self.tool_file_info(&arguments),
            "fmm_analyze_dependencies" => self.tool_dependency_graph(&arguments),
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

        let args: LookupExportArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        match manifest.export_index.get(&args.name) {
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
            None => Err(format!("Export '{}' not found", args.name)),
        }
    }

    fn tool_list_exports(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: ListExportsArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        if let Some(ref file_path) = args.file {
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
        } else if let Some(ref pat) = args.pattern {
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

        let args: FileInfoArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        match manifest.files.get(&args.file) {
            Some(entry) => {
                let result = json!({
                    "file": args.file,
                    "exports": entry.exports,
                    "imports": entry.imports,
                    "dependencies": entry.dependencies,
                    "loc": entry.loc,
                });
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
            }
            None => Err(format!("File '{}' not found in manifest", args.file)),
        }
    }

    fn tool_dependency_graph(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: DependencyGraphArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        let entry = manifest
            .files
            .get(&args.file)
            .ok_or_else(|| format!("File '{}' not found in manifest", args.file))?;

        // Upstream: files this file depends on (its dependencies)
        let upstream: Vec<&str> = entry.dependencies.iter().map(|s| s.as_str()).collect();

        // Downstream: files that depend on this file
        let mut downstream: Vec<&String> = manifest
            .files
            .iter()
            .filter(|(path, _)| path.as_str() != args.file)
            .filter(|(path, e)| {
                e.dependencies
                    .iter()
                    .any(|d| dep_matches(d, &args.file, path))
            })
            .map(|(path, _)| path)
            .collect();
        downstream.sort();

        let result = json!({
            "file": args.file,
            "upstream": upstream,
            "downstream": downstream,
            "imports": entry.imports,
        });
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn tool_search(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: SearchArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        let mut results: Vec<(&String, &crate::manifest::FileEntry)> = Vec::new();

        let has_export = args.export.is_some();
        let has_imports = args.imports.is_some();
        let has_depends_on = args.depends_on.is_some();

        // Search by export
        if let Some(ref export) = args.export {
            if let Some(file_path) = manifest.export_index.get(export.as_str()) {
                if let Some(entry) = manifest.files.get(file_path) {
                    results.push((file_path, entry));
                }
            }
        }

        // Search by imports
        if let Some(ref import_name) = args.imports {
            for (file_path, entry) in &manifest.files {
                if entry
                    .imports
                    .iter()
                    .any(|i| i.contains(import_name.as_str()))
                    && !results.iter().any(|(f, _)| *f == file_path)
                {
                    results.push((file_path, entry));
                }
            }
        }

        // Search by depends_on
        if let Some(ref dep_path) = args.depends_on {
            for (file_path, entry) in &manifest.files {
                if entry
                    .dependencies
                    .iter()
                    .any(|d| d.contains(dep_path.as_str()))
                    && !results.iter().any(|(f, _)| *f == file_path)
                {
                    results.push((file_path, entry));
                }
            }
        }

        if args.min_loc.is_some() || args.max_loc.is_some() {
            if results.is_empty() && !has_export && !has_imports && !has_depends_on {
                for (file_path, entry) in &manifest.files {
                    results.push((file_path, entry));
                }
            }

            results.retain(|(_, entry)| {
                let passes_min = args.min_loc.is_none_or(|min| entry.loc >= min);
                let passes_max = args.max_loc.is_none_or(|max| entry.loc <= max);
                passes_min && passes_max
            });
        }

        // If no filters, return all
        if !has_export
            && !has_imports
            && !has_depends_on
            && args.min_loc.is_none()
            && args.max_loc.is_none()
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
}

/// Check if a dependency path from `dependent_file` resolves to `target_file`.
/// Dependencies are stored as relative paths like "../utils/crypto.utils.js"
/// and need to be resolved against the dependent file's directory.
fn dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
    // Resolve the dependency path relative to the dependent file's directory
    let dep_dir = dependent_file
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");

    // Build resolved path by applying relative segments
    let mut parts: Vec<&str> = if dep_dir.is_empty() {
        Vec::new()
    } else {
        dep_dir.split('/').collect()
    };

    let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
    for segment in dep_clean.split('/') {
        if segment == ".." {
            parts.pop();
        } else if segment != "." {
            parts.push(segment);
        }
    }

    let resolved = parts.join("/");

    // Strip extension from both for comparison (.ts/.js/.tsx/.jsx interchangeable)
    let resolved_stem = resolved
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(&resolved);
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(target_file);

    resolved_stem == target_stem
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_matches_relative_path() {
        // dep "./types" from "src/index.ts" resolves to "src/types"
        assert!(dep_matches("./types", "src/types.ts", "src/index.ts"));
        assert!(dep_matches("./config", "src/config.ts", "src/index.ts"));
        assert!(!dep_matches("./types", "src/other.ts", "src/index.ts"));
    }

    #[test]
    fn dep_matches_nested_path() {
        // dep "./utils/helpers" from "src/index.ts" resolves to "src/utils/helpers"
        assert!(dep_matches(
            "./utils/helpers",
            "src/utils/helpers.ts",
            "src/index.ts"
        ));
        assert!(!dep_matches(
            "./utils/helpers",
            "src/utils/other.ts",
            "src/index.ts"
        ));
    }

    #[test]
    fn dep_matches_parent_relative() {
        // dep "../utils/crypto.utils.js" from "pkg/src/services/auth.service.ts"
        // resolves to "pkg/src/utils/crypto.utils"
        assert!(dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/services/auth.service.ts"
        ));
        assert!(!dep_matches(
            "../utils/crypto.utils.js",
            "pkg/src/services/other.ts",
            "pkg/src/services/auth.service.ts"
        ));
    }

    #[test]
    fn dep_matches_deep_parent_relative() {
        // dep "../../../utils/crypto.utils.js" from "pkg/src/tests/unit/auth/test.ts"
        // resolves to "pkg/src/utils/crypto.utils" (going up 3 dirs from tests/unit/auth)
        assert!(dep_matches(
            "../../../utils/crypto.utils.js",
            "pkg/src/utils/crypto.utils.ts",
            "pkg/src/tests/unit/auth/test.ts"
        ));
    }

    #[test]
    fn dep_matches_without_prefix() {
        assert!(dep_matches("types", "src/types.ts", "src/index.ts"));
    }

    #[test]
    fn test_server_construction() {
        let server = McpServer::new();
        assert!(server.root.is_absolute() || server.root.as_os_str().is_empty());
    }
}
