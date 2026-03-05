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
    term: Option<String>,
    export: Option<String>,
    imports: Option<String>,
    depends_on: Option<String>,
    min_loc: Option<usize>,
    max_loc: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ReadSymbolArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct FileOutlineArgs {
    file: String,
}

#[derive(Debug, Deserialize)]
struct ListFilesArgs {
    directory: Option<String>,
    pattern: Option<String>,
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
    const MAX_RESPONSE_BYTES: usize = 10_240;

    fn cap_response(text: String) -> String {
        if text.len() <= Self::MAX_RESPONSE_BYTES {
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
            "\n\n[Truncated — showing {}/{} lines. Use more specific filters.]",
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
                name: "fmm_read_symbol".to_string(),
                description: "Read the source code for a specific exported symbol. Returns the exact lines where the function/class/type is defined, without reading the entire file. Requires line-range data from v0.3 sidecars.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Exact export name to read (function, class, type, component)"
                        }
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "fmm_file_outline".to_string(),
                description: "Get a spatial outline of a file: every exported symbol with its line range and size. Like a table-of-contents for the file. Use to understand file structure before reading specific symbols.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "File path to outline — returns all exports with line ranges and sizes"
                        }
                    },
                    "required": ["file"]
                }),
            },
            Tool {
                name: "fmm_search".to_string(),
                description: "Universal codebase search. Use 'term' for smart search across exports, files, and imports (like 'fmm search <term>'). Use structured filters (export, imports, depends_on, LOC) for precise queries. Filters combine with AND logic.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "term": {
                            "type": "string",
                            "description": "Universal search term — searches exports (exact then fuzzy), file paths, and imports. Returns grouped results. Use alone without other filters."
                        },
                        "export": {
                            "type": "string",
                            "description": "Find files exporting this symbol (exact match, then case-insensitive substring fallback)"
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
            Tool {
                name: "fmm_list_files".to_string(),
                description: "List all indexed files under a directory prefix. The first tool to reach for when exploring an unknown module or package. Returns file paths with LOC and export count sorted alphabetically.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory prefix to filter files (e.g. 'src/cli/' or 'libs/agno/models'). Omit to list all indexed files."
                        },
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern to filter by filename within the directory (e.g. '*.py', '*.rs', 'test_*'). Supports * wildcard."
                        }
                    }
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
            // Original tools
            "fmm_lookup_export" => self.tool_lookup_export(&arguments),
            "fmm_list_exports" => self.tool_list_exports(&arguments),
            "fmm_file_info" => self.tool_file_info(&arguments),
            "fmm_dependency_graph" => self.tool_dependency_graph(&arguments),
            "fmm_search" => self.tool_search(&arguments),
            "fmm_read_symbol" => self.tool_read_symbol(&arguments),
            "fmm_file_outline" => self.tool_file_outline(&arguments),
            "fmm_list_files" => self.tool_list_files(&arguments),
            // Legacy aliases
            "fmm_find_export" => self.tool_lookup_export(&arguments),
            "fmm_find_symbol" => self.tool_lookup_export(&arguments),
            "fmm_file_metadata" => self.tool_file_info(&arguments),
            "fmm_analyze_dependencies" => self.tool_dependency_graph(&arguments),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(text) => {
                let text = Self::cap_response(text);
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }))
            }
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

        // Try export_locations first, then export_index for backward compat
        let (file, symbol_lines) = if let Some(loc) = manifest.export_locations.get(&args.name) {
            (loc.file.clone(), loc.lines.clone())
        } else if let Some(file_path) = manifest.export_index.get(&args.name) {
            (file_path.clone(), None)
        } else {
            return Err(format!("Export '{}' not found", args.name));
        };

        let entry = manifest
            .files
            .get(&file)
            .ok_or_else(|| format!("File '{}' not found in manifest", file))?;

        Ok(crate::format::format_lookup_export(
            &args.name,
            &file,
            symbol_lines.as_ref(),
            entry,
        ))
    }

    fn tool_list_exports(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: ListExportsArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        if let Some(ref file_path) = args.file {
            let entry = manifest
                .files
                .get(file_path)
                .ok_or_else(|| format!("File '{}' not found in manifest", file_path))?;
            Ok(crate::format::format_list_exports_file(file_path, entry))
        } else if let Some(ref pat) = args.pattern {
            let pat_lower = pat.to_lowercase();
            let mut matches: Vec<(String, String, Option<[usize; 2]>)> = manifest
                .export_index
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains(&pat_lower))
                .map(|(name, path)| {
                    let lines = manifest
                        .export_locations
                        .get(name)
                        .and_then(|loc| loc.lines.as_ref())
                        .map(|l| [l.start, l.end]);
                    (name.clone(), path.clone(), lines)
                })
                .collect();
            matches.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
            Ok(crate::format::format_list_exports_pattern(&matches))
        } else {
            let mut by_file: Vec<(&str, &crate::manifest::FileEntry)> = manifest
                .files
                .iter()
                .filter(|(_, entry)| !entry.exports.is_empty())
                .map(|(path, entry)| (path.as_str(), entry))
                .collect();
            by_file.sort_by_key(|(path, _)| path.to_lowercase());
            Ok(crate::format::format_list_exports_all(&by_file))
        }
    }

    fn tool_file_info(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: FileInfoArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        validate_not_directory(&args.file, &self.root)?;

        let entry = manifest.files.get(&args.file).ok_or_else(|| {
            format!(
                "File '{}' not found in manifest. Run 'fmm generate' to index the file.",
                args.file
            )
        })?;

        Ok(crate::format::format_file_info(&args.file, entry))
    }

    fn tool_dependency_graph(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: DependencyGraphArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        validate_not_directory(&args.file, &self.root)?;

        let entry = manifest.files.get(&args.file).ok_or_else(|| {
            format!(
                "File '{}' not found in manifest. Run 'fmm generate' to index the file.",
                args.file
            )
        })?;

        let (local, external, downstream) =
            crate::search::dependency_graph(manifest, &args.file, entry);

        Ok(crate::format::format_dependency_graph(
            &args.file,
            entry,
            &local,
            &external,
            &downstream,
        ))
    }

    fn tool_read_symbol(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: ReadSymbolArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        if args.name.trim().is_empty() {
            return Err("Symbol name must not be empty. Use fmm_list_exports to discover available symbols.".to_string());
        }

        let location = manifest
            .export_locations
            .get(&args.name)
            .ok_or_else(|| format!("Export '{}' not found. Use fmm_list_exports or fmm_search to discover available symbols.", args.name))?;

        // If the winning location is a re-export hub (index file), try to find the
        // concrete definition in a nearby non-index file that also exports this symbol.
        let (resolved_file, resolved_lines) = if is_reexport_file(&location.file) {
            if let Some((concrete_file, concrete_lines)) =
                find_concrete_definition(manifest, &args.name, &location.file)
            {
                (concrete_file, Some(concrete_lines))
            } else {
                // No concrete definition found — fall back to the re-export site
                (location.file.clone(), location.lines.clone())
            }
        } else {
            (location.file.clone(), location.lines.clone())
        };

        let lines = resolved_lines.ok_or_else(|| {
            format!(
                "No line range for '{}' in '{}' — regenerate sidecars with 'fmm generate' for v0.3 format",
                args.name, resolved_file,
            )
        })?;

        let source_path = self.root.join(&resolved_file);
        let content = std::fs::read_to_string(&source_path)
            .map_err(|e| format!("Cannot read '{}': {}", resolved_file, e))?;

        let source_lines: Vec<&str> = content.lines().collect();
        let start = lines.start.saturating_sub(1);
        let end = lines.end.min(source_lines.len());

        if start >= source_lines.len() {
            return Err(format!(
                "Line range [{}, {}] out of bounds for '{}' ({} lines)",
                lines.start,
                lines.end,
                resolved_file,
                source_lines.len()
            ));
        }

        let symbol_source = source_lines[start..end].join("\n");

        Ok(crate::format::format_read_symbol(
            &args.name,
            &resolved_file,
            &lines,
            &symbol_source,
        ))
    }

    fn tool_file_outline(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: FileOutlineArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        let entry = manifest
            .files
            .get(&args.file)
            .ok_or_else(|| format!("File '{}' not found in manifest", args.file))?;

        Ok(crate::format::format_file_outline(&args.file, entry))
    }

    fn tool_list_files(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: ListFilesArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        let dir = args.directory.as_deref();
        let pat = args.pattern.as_deref();

        let mut entries: Vec<(&str, usize, usize)> = manifest
            .files
            .iter()
            .filter(|(path, _)| {
                if let Some(d) = dir {
                    if !path.starts_with(d) {
                        return false;
                    }
                }
                if let Some(p) = pat {
                    let filename = path.rsplit('/').next().unwrap_or(path.as_str());
                    if !glob_filename_matches(p, filename) {
                        return false;
                    }
                }
                true
            })
            .map(|(path, entry)| (path.as_str(), entry.loc, entry.exports.len()))
            .collect();

        entries.sort_by_key(|(path, _, _)| path.to_lowercase());

        Ok(crate::format::format_list_files(dir, &entries))
    }

    fn tool_search(&self, args: &Value) -> Result<String, String> {
        let manifest = self.require_manifest()?;

        let args: SearchArgs =
            serde_json::from_value(args.clone()).map_err(|e| format!("Invalid arguments: {e}"))?;

        // Universal term search
        if let Some(ref term) = args.term {
            let result = crate::search::bare_search(manifest, term);
            return Ok(crate::format::format_bare_search(&result, false));
        }

        // Structured filter search
        let filters = crate::search::SearchFilters {
            export: args.export,
            imports: args.imports,
            depends_on: args.depends_on,
            min_loc: args.min_loc,
            max_loc: args.max_loc,
        };
        let results = crate::search::filter_search(manifest, &filters);
        Ok(crate::format::format_filter_search(&results, false))
    }
}

/// Return true if a file path is a conventional re-export hub (index/init file).
/// These files aggregate symbols from sub-modules and are not the definition site.
fn is_reexport_file(file_path: &str) -> bool {
    let filename = file_path.rsplit('/').next().unwrap_or(file_path);
    matches!(
        filename,
        "__init__.py" | "index.ts" | "index.tsx" | "index.js" | "index.jsx" | "mod.rs"
    )
}

/// Given that `symbol` was found in a re-export hub, search the manifest for a
/// non-index file that also exports the same symbol, preferring files whose
/// directory path shares the most prefix with `reexport_file`.
///
/// Returns `(concrete_file_path, ExportLines)` or `None` if no candidate found.
fn find_concrete_definition(
    manifest: &crate::manifest::Manifest,
    symbol: &str,
    reexport_file: &str,
) -> Option<(String, crate::manifest::ExportLines)> {
    let reexport_dir = reexport_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

    let mut candidates: Vec<(String, crate::manifest::ExportLines, usize)> = manifest
        .files
        .iter()
        .filter(|(path, _)| {
            let p = path.as_str();
            p != reexport_file && !is_reexport_file(p)
        })
        .filter_map(|(path, entry)| {
            // Find this symbol in the file's export list
            let idx = entry.exports.iter().position(|e| e == symbol)?;
            // Require line-range data — without it we cannot show source
            let lines = entry
                .export_lines
                .as_ref()
                .and_then(|el| el.get(idx))
                .filter(|l| l.start > 0)?;
            // Shared prefix length as proximity score
            let file_dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let shared = reexport_dir
                .chars()
                .zip(file_dir.chars())
                .take_while(|(a, b)| a == b)
                .count();
            Some((path.clone(), lines.clone(), shared))
        })
        .collect();

    // Sort by proximity descending so closest sibling wins
    candidates.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));
    candidates.into_iter().map(|(f, l, _)| (f, l)).next()
}

/// Return an error if `path` looks like a directory (ends with `/` or resolves to a dir on disk).
/// Provides a helpful message pointing to fmm_list_files.
fn validate_not_directory(path: &str, root: &std::path::Path) -> Result<(), String> {
    if path.ends_with('/') || path.ends_with(std::path::MAIN_SEPARATOR) {
        return Err(format!(
            "'{}' is a directory, not a file. Use fmm_list_files(directory: \"{}\") to list its contents.",
            path, path
        ));
    }
    let resolved = root.join(path);
    if resolved.is_dir() {
        return Err(format!(
            "'{}' is a directory, not a file. Use fmm_list_files(directory: \"{}/\") to list its contents.",
            path, path
        ));
    }
    Ok(())
}

/// Match a glob pattern against a filename (last path component).
/// Supports `*` as a wildcard within the filename. Does not match path separators.
/// Examples: `*.py`, `test_*`, `*_test.rs`, `*`
fn glob_filename_matches(pattern: &str, filename: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return filename == pattern;
    }
    // Split on the first `*` and check prefix + suffix
    let (prefix, rest) = pattern.split_once('*').unwrap();
    if !filename.starts_with(prefix) {
        return false;
    }
    let after_prefix = &filename[prefix.len()..];
    // Handle remaining pattern segments (multiple `*`)
    if rest.contains('*') {
        // Recursively match the remainder
        glob_filename_matches(rest, after_prefix)
    } else {
        // Single `*` — remainder is a literal suffix
        after_prefix.ends_with(rest) && after_prefix.len() >= rest.len()
    }
}

/// Check if a dependency path from `dependent_file` resolves to `target_file`.
/// Dependencies are stored as relative paths like "../utils/crypto.utils.js"
/// and need to be resolved against the dependent file's directory.
pub fn dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
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

    #[test]
    fn cap_response_handles_multibyte_utf8() {
        // Build a string that would split a multi-byte char at MAX_RESPONSE_BYTES
        let prefix = "x".repeat(McpServer::MAX_RESPONSE_BYTES - 1);
        // 4-byte emoji straddles the boundary
        let text = format!("{}🦀 and more text after", prefix);
        let result = McpServer::cap_response(text);
        assert!(result.is_char_boundary(result.len()));
        assert!(result.contains("[Truncated"));
    }

    #[test]
    fn cap_response_passes_through_short_text() {
        let short = "hello world".to_string();
        assert_eq!(McpServer::cap_response(short.clone()), short);
    }

    #[test]
    fn file_info_directory_path_returns_helpful_error() {
        use crate::manifest::Manifest;
        let server = McpServer {
            manifest: Some(Manifest::new()),
            root: std::path::PathBuf::from("/tmp"),
        };
        let result = server
            .call_tool("fmm_file_info", serde_json::json!({"file": "src/cli/"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            result["isError"].as_bool().unwrap_or(false),
            "expected isError"
        );
        assert!(
            text.contains("fmm_list_files"),
            "should suggest fmm_list_files, got: {}",
            text
        );
    }

    #[test]
    fn dependency_graph_directory_path_returns_helpful_error() {
        use crate::manifest::Manifest;
        let server = McpServer {
            manifest: Some(Manifest::new()),
            root: std::path::PathBuf::from("/tmp"),
        };
        let result = server
            .call_tool(
                "fmm_dependency_graph",
                serde_json::json!({"file": "src/mcp/"}),
            )
            .unwrap();
        assert!(
            result["isError"].as_bool().unwrap_or(false),
            "expected isError"
        );
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("fmm_list_files"),
            "should suggest fmm_list_files, got: {}",
            text
        );
    }

    #[test]
    fn read_symbol_empty_name_returns_helpful_error() {
        use crate::manifest::Manifest;
        let server = McpServer {
            manifest: Some(Manifest::new()),
            root: std::path::PathBuf::from("/tmp"),
        };
        let result = server
            .call_tool("fmm_read_symbol", serde_json::json!({"name": ""}))
            .unwrap();
        assert!(
            result["isError"].as_bool().unwrap_or(false),
            "expected isError"
        );
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("fmm_list_exports"),
            "should suggest fmm_list_exports, got: {}",
            text
        );
    }

    #[test]
    fn is_reexport_file_detects_index_files() {
        assert!(is_reexport_file("agno/__init__.py"));
        assert!(is_reexport_file("src/index.ts"));
        assert!(is_reexport_file("src/index.tsx"));
        assert!(is_reexport_file("src/mod.rs"));
        assert!(is_reexport_file("libs/foo/index.js"));
        assert!(!is_reexport_file("agno/agent/agent.py"));
        assert!(!is_reexport_file("src/auth.ts"));
    }

    #[test]
    fn read_symbol_follows_reexport_to_concrete_definition() {
        use crate::manifest::Manifest;
        use crate::parser::{ExportEntry, Metadata};

        // Create a temp dir with actual source files
        let dir = tempfile::tempdir().unwrap();
        let init_path = dir.path().join("agno").join("__init__.py");
        let agent_path = dir.path().join("agno").join("agent").join("agent.py");
        std::fs::create_dir_all(agent_path.parent().unwrap()).unwrap();

        // __init__.py re-exports Agent
        std::fs::write(
            &init_path,
            "from .agent.agent import Agent\n__all__ = ['Agent']\n",
        )
        .unwrap();

        // agent.py is the concrete definition with 5 lines
        let agent_src = "class Agent:\n    def __init__(self):\n        pass\n    def run(self):\n        pass\n";
        std::fs::write(&agent_path, agent_src).unwrap();

        let mut manifest = Manifest::new();
        // Index file re-exports Agent (no line range — typical for re-exports)
        manifest.add_file(
            "agno/__init__.py",
            Metadata {
                exports: vec![ExportEntry::new("Agent".to_string(), 1, 1)],
                imports: vec!["agno.agent.agent".to_string()],
                dependencies: vec![],
                loc: 2,
            },
        );
        // Concrete definition with proper line range
        manifest.add_file(
            "agno/agent/agent.py",
            Metadata {
                exports: vec![ExportEntry::new("Agent".to_string(), 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 5,
            },
        );

        // __init__.py wins the export_index (last writer wins), but we want agent.py
        let server = McpServer {
            manifest: Some(manifest),
            root: dir.path().to_path_buf(),
        };

        let result = server
            .call_tool("fmm_read_symbol", serde_json::json!({"name": "Agent"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();

        // Should resolve to the concrete definition file, not __init__.py
        assert!(
            text.contains("agno/agent/agent.py"),
            "should resolve to concrete definition, got: {}",
            text
        );
        assert!(
            !text.contains("__init__.py"),
            "should not use re-export site, got: {}",
            text
        );
        assert!(
            text.contains("class Agent"),
            "should include class body, got: {}",
            text
        );
    }

    #[test]
    fn glob_filename_matches_star_ext() {
        assert!(glob_filename_matches("*.py", "agent.py"));
        assert!(glob_filename_matches("*.rs", "mod.rs"));
        assert!(!glob_filename_matches("*.py", "agent.rs"));
        assert!(!glob_filename_matches("*.py", "agent.pyc"));
    }

    #[test]
    fn glob_filename_matches_prefix_star() {
        assert!(glob_filename_matches("test_*", "test_agent.py"));
        assert!(glob_filename_matches("test_*", "test_.py"));
        assert!(!glob_filename_matches("test_*", "mytest_agent.py"));
    }

    #[test]
    fn glob_filename_matches_literal() {
        assert!(glob_filename_matches("mod.rs", "mod.rs"));
        assert!(!glob_filename_matches("mod.rs", "mod.ts"));
    }

    #[test]
    fn glob_filename_matches_star_wildcard() {
        assert!(glob_filename_matches("*", "anything.py"));
        assert!(glob_filename_matches("*", ""));
    }

    #[test]
    fn list_files_tool_no_args() {
        use crate::manifest::Manifest;
        use crate::parser::{ExportEntry, Metadata};

        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/a.rs",
            Metadata {
                exports: vec![ExportEntry::new("Foo".to_string(), 1, 10)],
                imports: vec![],
                dependencies: vec![],
                loc: 50,
            },
        );
        manifest.add_file(
            "src/b.rs",
            Metadata {
                exports: vec![],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
            },
        );

        let server = McpServer {
            manifest: Some(manifest),
            root: std::path::PathBuf::from("/tmp"),
        };

        let result = server
            .call_tool("fmm_list_files", serde_json::json!({}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("total: 2"),
            "expected total: 2, got: {}",
            text
        );
        assert!(text.contains("src/a.rs"));
        assert!(text.contains("src/b.rs"));
    }

    #[test]
    fn list_files_tool_with_directory() {
        use crate::manifest::Manifest;
        use crate::parser::{ExportEntry, Metadata};

        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/cli/mod.rs",
            Metadata {
                exports: vec![ExportEntry::new("Cli".to_string(), 1, 5)],
                imports: vec![],
                dependencies: vec![],
                loc: 30,
            },
        );
        manifest.add_file(
            "src/mcp/mod.rs",
            Metadata {
                exports: vec![],
                imports: vec![],
                dependencies: vec![],
                loc: 100,
            },
        );

        let server = McpServer {
            manifest: Some(manifest),
            root: std::path::PathBuf::from("/tmp"),
        };

        let result = server
            .call_tool(
                "fmm_list_files",
                serde_json::json!({"directory": "src/cli/"}),
            )
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("total: 1"), "got: {}", text);
        assert!(text.contains("src/cli/mod.rs"));
        assert!(!text.contains("src/mcp/mod.rs"));
    }
}
