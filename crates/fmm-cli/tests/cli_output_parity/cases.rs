use serde_json::{Value, json};

use super::Projection;

pub(super) struct ParityCase {
    pub(super) name: &'static str,
    pub(super) args: &'static [&'static str],
    pub(super) projection: Projection,
}

pub(super) struct McpParityCase {
    pub(super) name: &'static str,
    pub(super) cli_args: &'static [&'static str],
    pub(super) tool: &'static str,
    pub(super) arguments: Value,
    pub(super) projection: Projection,
}

pub(super) fn parity_cases() -> Vec<ParityCase> {
    vec![
        ParityCase {
            name: "lookup",
            args: &["lookup", "ParserRegistry"],
            projection: Projection::Lookup,
        },
        ParityCase {
            name: "exports-file",
            args: &["exports", "--file", "crates/fmm-core/src/parser/mod.rs"],
            projection: Projection::ExportsFile,
        },
        ParityCase {
            name: "exports-pattern",
            args: &["exports", "ParserRegistry", "--limit", "5"],
            projection: Projection::ExportsPattern,
        },
        ParityCase {
            name: "outline",
            args: &["outline", "crates/fmm-core/src/parser/mod.rs"],
            projection: Projection::Outline,
        },
        ParityCase {
            name: "deps",
            args: &["deps", "crates/fmm-core/src/parser/mod.rs"],
            projection: Projection::Deps,
        },
        ParityCase {
            name: "ls",
            args: &[
                "ls",
                "crates/fmm-core/src/parser",
                "--sort-by",
                "name",
                "--limit",
                "3",
            ],
            projection: Projection::Ls,
        },
        ParityCase {
            name: "read",
            args: &["read", "ParserRegistry"],
            projection: Projection::Read,
        },
        ParityCase {
            name: "search-bare",
            args: &["search", "ParserRegistry", "--limit", "3"],
            projection: Projection::SearchBare,
        },
        ParityCase {
            name: "search-export",
            args: &["search", "--export", "ParserRegistry", "--limit", "3"],
            projection: Projection::SearchExport,
        },
        ParityCase {
            name: "search-filter",
            args: &["search", "--imports", "serde", "--min-loc", "600"],
            projection: Projection::SearchFilter,
        },
        ParityCase {
            name: "glossary",
            args: &["glossary", "ParserRegistry", "--limit", "3"],
            projection: Projection::Glossary,
        },
    ]
}

pub(super) fn mcp_parity_cases() -> Vec<McpParityCase> {
    vec![
        McpParityCase {
            name: "mcp-lookup",
            cli_args: &["lookup", "ParserRegistry"],
            tool: "fmm_lookup_export",
            arguments: json!({"name": "ParserRegistry"}),
            projection: Projection::Lookup,
        },
        McpParityCase {
            name: "mcp-list-exports-file",
            cli_args: &["exports", "--file", "crates/fmm-core/src/parser/mod.rs"],
            tool: "fmm_list_exports",
            arguments: json!({"file": "crates/fmm-core/src/parser/mod.rs"}),
            projection: Projection::ExportsFile,
        },
        McpParityCase {
            name: "mcp-list-exports-pattern",
            cli_args: &["exports", "ParserRegistry", "--limit", "5"],
            tool: "fmm_list_exports",
            arguments: json!({"pattern": "ParserRegistry", "limit": 5}),
            projection: Projection::ExportsPattern,
        },
        McpParityCase {
            name: "mcp-file-outline",
            cli_args: &["outline", "crates/fmm-core/src/parser/mod.rs"],
            tool: "fmm_file_outline",
            arguments: json!({"file": "crates/fmm-core/src/parser/mod.rs"}),
            projection: Projection::Outline,
        },
        McpParityCase {
            name: "mcp-dependency-graph",
            cli_args: &["deps", "crates/fmm-core/src/parser/mod.rs"],
            tool: "fmm_dependency_graph",
            arguments: json!({"file": "crates/fmm-core/src/parser/mod.rs"}),
            projection: Projection::Deps,
        },
        McpParityCase {
            name: "mcp-list-files",
            cli_args: &[
                "ls",
                "crates/fmm-core/src/parser",
                "--sort-by",
                "name",
                "--limit",
                "3",
            ],
            tool: "fmm_list_files",
            arguments: json!({
                "directory": "crates/fmm-core/src/parser",
                "sort_by": "name",
                "limit": 3
            }),
            projection: Projection::Ls,
        },
        McpParityCase {
            name: "mcp-read-symbol",
            cli_args: &["read", "ParserRegistry"],
            tool: "fmm_read_symbol",
            arguments: json!({"name": "ParserRegistry"}),
            projection: Projection::Read,
        },
        McpParityCase {
            name: "mcp-search-bare",
            cli_args: &["search", "ParserRegistry", "--limit", "3"],
            tool: "fmm_search",
            arguments: json!({"term": "ParserRegistry", "limit": 3}),
            projection: Projection::SearchBare,
        },
        McpParityCase {
            name: "mcp-search-filter",
            cli_args: &["search", "--imports", "serde", "--min-loc", "600"],
            tool: "fmm_search",
            arguments: json!({"imports": "serde", "min_loc": 600}),
            projection: Projection::SearchFilter,
        },
        McpParityCase {
            name: "mcp-glossary",
            cli_args: &["glossary", "ParserRegistry", "--limit", "3"],
            tool: "fmm_glossary",
            arguments: json!({"pattern": "ParserRegistry", "limit": 3}),
            projection: Projection::Glossary,
        },
    ]
}
