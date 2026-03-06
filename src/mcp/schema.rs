use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

pub(super) fn tool_list() -> Value {
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
            description: "Search or list exported symbols across the codebase. Use 'pattern' for fuzzy discovery (e.g. 'auth' matches validateAuth, authMiddleware). Use 'directory' to scope results to a path prefix (e.g. 'packages/core/'). Use 'file' to list a specific file's exports. Default limit: 200. Use offset to page through large result sets.".to_string(),
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
                    },
                    "directory": {
                        "type": "string",
                        "description": "Path prefix to scope results (e.g. 'packages/core/'). Only exports from files under this directory are returned."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 200). Increase for broader listings."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of results to skip before returning (default: 0). Use for pagination: offset=200 returns results 201–400."
                    }
                }
            }),
        },
        Tool {
            name: "fmm_dependency_graph".to_string(),
            description: "Get a file's dependency graph: upstream dependencies (what it imports) and downstream dependents (what would break if it changes). Use for impact analysis and blast radius. Add depth>1 for transitive traversal; depth=-1 for full closure.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "File path to analyze — returns all upstream dependencies and downstream dependents"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Traversal depth (default: 1 = direct deps only). depth=2 adds transitive deps. depth=-1 computes the full transitive closure. depth>1 returns flat lists with a depth annotation per entry."
                    }
                },
                "required": ["file"]
            }),
        },
        Tool {
            name: "fmm_read_symbol".to_string(),
            description: "Read the source code for a specific exported symbol. Returns the exact lines where the function/class/type is defined, without reading the entire file. Requires line-range data from v0.3 sidecars. Use `ClassName.method` notation to read a specific public method: `fmm_read_symbol(name: \"NestFactoryStatic.createApplicationContext\")`. For large symbols (>10KB) use truncate: false to get the full source.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact export name to read (function, class, type, component), or ClassName.method for a specific public method"
                    },
                    "truncate": {
                        "type": "boolean",
                        "description": "Whether to apply the 10KB response cap (default: true). Set to false to return the full source for large symbols that would otherwise be truncated."
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
            description: "Universal codebase search. Use 'term' for smart search across exports, files, and imports. Use structured filters (export, imports, depends_on, LOC) for precise queries. Combine 'term' with filters to narrow results with AND semantics — only exports matching the term from files matching the filters are returned. Note: depends_on uses transitive matching (full import chain), not direct-only. For direct importers only, use fmm_dependency_graph with depth=1.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "term": {
                        "type": "string",
                        "description": "Universal search term — searches exports (exact then fuzzy), file paths, and imports. Returns grouped results. Can be combined with structured filters to narrow results to matching files."
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
                        "description": "Find all files that transitively depend on this local path (full import chain, not just direct importers) — use for full blast radius before renaming. For direct-only importers, use fmm_dependency_graph with depth=1."
                    },
                    "min_loc": {
                        "type": "integer",
                        "description": "Minimum lines of code — find files larger than this"
                    },
                    "max_loc": {
                        "type": "integer",
                        "description": "Maximum lines of code — find files smaller than this"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of fuzzy export results (default: 50). Increase for broader searches."
                    }
                }
            }),
        },
        Tool {
            name: "fmm_list_files".to_string(),
            description: "List all indexed files under a directory prefix. The first tool to reach for when exploring an unknown module or package. Returns file paths with LOC, export count, and downstream dependent count. Default sort: LOC descending (largest files first). sort_by options: loc (default), name, exports, downstream (blast-radius sort), modified (most recently changed first). Default limit: 200. Use offset to page through large directories.".to_string(),
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
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of files to return (default: 200). Increase for broader listings."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of files to skip before returning results (default: 0). Use for pagination: offset=200 returns files 201–400."
                    },
                    "sort_by": {
                        "type": "string",
                        "enum": ["name", "loc", "exports", "downstream", "modified"],
                        "description": "Sort field. 'loc' (default): lines of code descending. 'name': alphabetical. 'exports': export count descending. 'downstream': direct dependent count descending (blast-radius sort — highest-risk files first). 'modified': most recently changed files first (date shown in output). 'loc', 'exports', 'downstream', and 'modified' default to descending order."
                    },
                    "order": {
                        "type": "string",
                        "enum": ["asc", "desc"],
                        "description": "Sort order. Defaults: 'name' → asc, 'loc'/'exports'/'downstream' → desc. Explicit 'asc'/'desc' overrides the default."
                    },
                    "group_by": {
                        "type": "string",
                        "enum": ["subdir"],
                        "description": "Collapse files into directory buckets. 'subdir': group by immediate subdirectory, showing file count and total LOC per bucket. Best for initial orientation of large codebases. sort_by applies to bucket-level LOC."
                    },
                    "filter": {
                        "type": "string",
                        "enum": ["all", "source", "tests"],
                        "description": "File type filter. 'all' (default): no filtering. 'source': exclude test files. 'tests': return only test files. Detection heuristic uses path segments (/test/, /e2e/, /__tests__/) and filename suffixes (.spec.ts, .test.ts, _test.go, etc.), configurable via test_patterns in .fmmrc.json."
                    }
                }
            }),
        },
        Tool {
            name: "fmm_glossary".to_string(),
            description: "Symbol-level impact analysis. Given a symbol name or pattern, returns all definitions and exactly which files import each one. Two modes controlled by the pattern: bare name (e.g. 'loadInstance') returns file-level used_by — all files that import the symbol's file; dotted name (e.g. 'Injector.loadInstance') adds call-site precision — a second tree-sitter pass filters to files that actually call that method. Use before renaming or changing a signature to get a precise blast radius — more surgical than fmm_dependency_graph which only gives file-level downstream.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Required. Case-insensitive substring filter on export name. Bare name (e.g. 'loadInstance') → file-level used_by. Dotted name (e.g. 'Injector.loadInstance') → call-site precision, filtered to actual callers."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max entries returned (default 10, hard cap at 50). Use a specific pattern to stay under the default."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["source", "tests", "all"],
                        "description": "source (default): excludes test symbols and test files — exact callers for refactoring. tests: only test exports — what tests exercise this symbol? all: unfiltered."
                    }
                },
                "required": ["pattern"]
            }),
        },
    ];

    json!({ "tools": tools })
}
