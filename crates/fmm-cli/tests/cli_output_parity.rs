use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
enum Projection {
    Deps,
    ExportsFile,
    ExportsPattern,
    Glossary,
    Lookup,
    Ls,
    Outline,
    Read,
    SearchBare,
    SearchExport,
    SearchFilter,
}

struct ParityCase {
    name: &'static str,
    args: &'static [&'static str],
    projection: Projection,
}

struct McpParityCase {
    name: &'static str,
    cli_args: &'static [&'static str],
    tool: &'static str,
    arguments: Value,
    projection: Projection,
}

#[test]
fn cli_text_and_json_outputs_have_semantic_parity() {
    ensure_repo_index();

    for case in parity_cases() {
        let text_output = run_fmm(case.args);
        let mut json_args = case.args.to_vec();
        json_args.push("--json");
        let json_output = run_fmm(&json_args);

        let text = normalize_text(case.projection, &text_output.stdout);
        let json = normalize_json(case.projection, &json_output.stdout);

        assert_eq!(
            text,
            json,
            "{} text/json parity mismatch\ntext:\n{}\njson:\n{}",
            case.name,
            String::from_utf8_lossy(&text_output.stdout),
            String::from_utf8_lossy(&json_output.stdout)
        );
    }
}

#[test]
fn mcp_text_and_cli_json_outputs_have_semantic_parity() {
    ensure_repo_index();
    let server = fmm::mcp::SqliteMcpServer::with_root(repo_root());

    for case in mcp_parity_cases() {
        let mcp_text = call_mcp_text(&server, case.tool, case.arguments);
        let mut json_args = case.cli_args.to_vec();
        json_args.push("--json");
        let json_output = run_fmm(&json_args);

        let mcp = normalize_text(case.projection, mcp_text.as_bytes());
        let json = normalize_json(case.projection, &json_output.stdout);

        assert_eq!(
            mcp,
            json,
            "{} MCP/CLI JSON parity mismatch\nmcp:\n{}\njson:\n{}",
            case.name,
            mcp_text,
            String::from_utf8_lossy(&json_output.stdout)
        );
    }
}

fn parity_cases() -> Vec<ParityCase> {
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

fn mcp_parity_cases() -> Vec<McpParityCase> {
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

fn ensure_repo_index() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let output = Command::cargo_bin("fmm")
            .unwrap()
            .arg("generate")
            .current_dir(repo_root())
            .output()
            .expect("failed to run fmm generate");
        assert!(
            output.status.success(),
            "fmm generate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

fn call_mcp_text(server: &fmm::mcp::SqliteMcpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn run_fmm(args: &[&str]) -> Output {
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("failed to run fmm");
    assert!(
        output.status.success(),
        "fmm {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under repo/crates/fmm-cli")
        .to_path_buf()
}

fn normalize_text(projection: Projection, stdout: &[u8]) -> Value {
    let raw = String::from_utf8_lossy(stdout);
    let text = strip_ansi(&raw);
    match projection {
        Projection::Deps => normalize_deps_text(&text),
        Projection::ExportsFile => normalize_exports_file_text(&text),
        Projection::ExportsPattern => Value::Array(parse_export_table(&text)),
        Projection::Glossary => normalize_glossary_text(&text),
        Projection::Lookup => normalize_lookup_text(&text),
        Projection::Ls => Value::Array(parse_ls_rows(&text)),
        Projection::Outline => normalize_outline_text(&text),
        Projection::Read => normalize_read_text(&text),
        Projection::SearchBare => normalize_search_bare_text(&text),
        Projection::SearchExport => Value::Array(parse_export_table(&text)),
        Projection::SearchFilter => {
            Value::Array(sort_objects_by_file(parse_search_filter_text(&text)))
        }
    }
}

fn normalize_json(projection: Projection, stdout: &[u8]) -> Value {
    let value: Value = serde_json::from_slice(stdout).expect("valid JSON output");
    match projection {
        Projection::Deps => object([
            ("file", value["file"].clone()),
            (
                "local_deps",
                value
                    .get("local_deps")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            ),
            ("external", value["external"].clone()),
            ("downstream", value["downstream"].clone()),
        ]),
        Projection::ExportsFile => object([
            ("file", value["file"].clone()),
            ("exports", normalize_export_array(&value["exports"], false)),
        ]),
        Projection::ExportsPattern | Projection::SearchExport => {
            normalize_export_array(&value, true)
        }
        Projection::Glossary => normalize_glossary_json(&value),
        Projection::Lookup => object([
            ("symbol", value["symbol"].clone()),
            ("file", value["file"].clone()),
            ("lines", value["lines"].clone()),
            ("exports", normalize_export_array(&value["exports"], false)),
            ("imports", value["imports"].clone()),
            ("dependencies", value["dependencies"].clone()),
            ("loc", value["loc"].clone()),
        ]),
        Projection::Ls => Value::Array(
            value
                .as_array()
                .expect("ls JSON should be an array")
                .iter()
                .map(|entry| {
                    object([
                        ("file", entry["file"].clone()),
                        ("loc", entry["loc"].clone()),
                        ("exports", entry["exports"].clone()),
                        ("downstream", entry["downstream"].clone()),
                    ])
                })
                .collect(),
        ),
        Projection::Outline => object([
            ("file", value["file"].clone()),
            (
                "imports",
                value.get("imports").cloned().unwrap_or_else(|| json!([])),
            ),
            (
                "dependencies",
                value
                    .get("dependencies")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            ),
            ("exports", normalize_export_array(&value["exports"], false)),
            ("loc", value["loc"].clone()),
        ]),
        Projection::Read => object([
            ("symbol", value["symbol"].clone()),
            ("file", value["file"].clone()),
            ("lines", value["lines"].clone()),
            ("source", value["source"].clone()),
        ]),
        Projection::SearchBare => object([
            ("exports", normalize_export_array(&value["exports"], true)),
            ("files", value["files"].clone()),
            ("imports", normalize_import_hits(&value["imports"])),
            (
                "named_imports",
                normalize_named_import_hits(&value["named_imports"]),
            ),
        ]),
        Projection::SearchFilter => Value::Array(sort_objects_by_file(
            value
                .as_array()
                .expect("search filter JSON should be an array")
                .iter()
                .map(normalize_search_filter_json_entry)
                .collect(),
        )),
    }
}

fn normalize_lookup_text(text: &str) -> Value {
    object([
        ("symbol", scalar_field(text, "symbol")),
        ("file", scalar_field(text, "file")),
        ("lines", array_field(text, "lines").unwrap()),
        (
            "exports",
            Value::Array(parse_named_lines_block(text, "exports")),
        ),
        (
            "imports",
            array_field(text, "imports").unwrap_or_else(|| json!([])),
        ),
        (
            "dependencies",
            array_field(text, "dependencies").unwrap_or_else(|| json!([])),
        ),
        ("loc", scalar_field(text, "loc")),
    ])
}

fn normalize_exports_file_text(text: &str) -> Value {
    object([
        ("file", scalar_field(text, "file")),
        (
            "exports",
            Value::Array(parse_named_lines_block(text, "exports")),
        ),
    ])
}

fn normalize_outline_text(text: &str) -> Value {
    object([
        ("file", scalar_field(text, "file")),
        (
            "imports",
            array_field(text, "imports").unwrap_or_else(|| json!([])),
        ),
        (
            "dependencies",
            array_field(text, "dependencies").unwrap_or_else(|| json!([])),
        ),
        (
            "exports",
            Value::Array(parse_named_lines_block(text, "symbols")),
        ),
        ("loc", scalar_field(text, "loc")),
    ])
}

fn normalize_deps_text(text: &str) -> Value {
    object([
        ("file", scalar_field(text, "file")),
        (
            "local_deps",
            array_field(text, "local_deps")
                .unwrap_or_else(|| Value::Array(parse_list_block(text, "local_deps"))),
        ),
        (
            "external",
            array_field(text, "external").unwrap_or_else(|| json!([])),
        ),
        (
            "downstream",
            Value::Array(parse_list_block(text, "downstream")),
        ),
    ])
}

fn normalize_read_text(text: &str) -> Value {
    let mut parts = text.splitn(3, "---");
    let _before = parts.next();
    let header = parts.next().unwrap_or_default();
    let source = parts.next().unwrap_or_default().trim_start_matches('\n');
    object([
        ("symbol", scalar_field(header, "symbol")),
        ("file", scalar_field(header, "file")),
        ("lines", array_field(header, "lines").unwrap()),
        ("source", Value::String(source.trim_end().to_string())),
    ])
}

fn normalize_search_bare_text(text: &str) -> Value {
    let sections = search_sections(text);
    object([
        (
            "exports",
            Value::Array(
                sections
                    .get("EXPORTS")
                    .map(|s| parse_export_table(s))
                    .unwrap_or_default(),
            ),
        ),
        (
            "files",
            Value::Array(
                sections
                    .get("FILES")
                    .map(|s| {
                        s.lines()
                            .filter_map(|line| line.trim().strip_prefix(""))
                            .filter(|line| !line.is_empty())
                            .map(|line| Value::String(line.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
        ),
        (
            "imports",
            Value::Array(
                sections
                    .get("IMPORTS")
                    .map(|s| parse_import_hits(s))
                    .unwrap_or_default(),
            ),
        ),
        (
            "named_imports",
            Value::Array(
                sections
                    .get("NAMED IMPORTS")
                    .map(|s| parse_named_import_hits(s))
                    .unwrap_or_default(),
            ),
        ),
    ])
}

fn normalize_glossary_text(text: &str) -> Value {
    let mut entries = Vec::new();
    let mut current_entry: Option<Map<String, Value>> = None;
    let mut current_sources: Vec<Value> = Vec::new();
    let mut current_source: Option<Map<String, Value>> = None;
    let mut in_used_by = false;

    let flush_source = |current_source: &mut Option<Map<String, Value>>,
                        current_sources: &mut Vec<Value>| {
        if let Some(mut source) = current_source.take() {
            source
                .entry("used_by".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            current_sources.push(Value::Object(source));
        }
    };

    let flush_entry = |current_entry: &mut Option<Map<String, Value>>,
                       current_sources: &mut Vec<Value>,
                       entries: &mut Vec<Value>| {
        if let Some(mut entry) = current_entry.take() {
            entry.insert(
                "sources".to_string(),
                Value::Array(std::mem::take(current_sources)),
            );
            entries.push(Value::Object(entry));
        }
    };

    for raw in text.lines() {
        let line = strip_comment(raw).trim_end().to_string();
        if line.trim().is_empty() || line == "---" {
            continue;
        }
        if !line.starts_with(' ') && line.ends_with(':') {
            flush_source(&mut current_source, &mut current_sources);
            flush_entry(&mut current_entry, &mut current_sources, &mut entries);
            let mut entry = Map::new();
            entry.insert(
                "name".to_string(),
                Value::String(line.trim_end_matches(':').to_string()),
            );
            current_entry = Some(entry);
            in_used_by = false;
        } else if let Some(rest) = line.trim_start().strip_prefix("- src: ") {
            flush_source(&mut current_source, &mut current_sources);
            current_source = Some(parse_glossary_source(rest));
            in_used_by = false;
        } else if let Some(rest) = line.trim_start().strip_prefix("used_by:") {
            let callers = if let Some(value) = rest.trim().strip_prefix('[') {
                parse_inline_array(&format!("[{}", value))
            } else {
                Vec::new()
            };
            if let Some(source) = &mut current_source {
                source.insert("used_by".to_string(), Value::Array(callers));
            }
            in_used_by = true;
        } else if in_used_by
            && let Some(caller) = line.trim_start().strip_prefix("- ")
            && let Some(source) = &mut current_source
        {
            let used_by = source
                .entry("used_by".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            used_by
                .as_array_mut()
                .unwrap()
                .push(Value::String(caller.to_string()));
        }
    }

    flush_source(&mut current_source, &mut current_sources);
    flush_entry(&mut current_entry, &mut current_sources, &mut entries);
    Value::Array(entries)
}

fn normalize_glossary_json(value: &Value) -> Value {
    Value::Array(
        value
            .as_array()
            .expect("glossary JSON should be an array")
            .iter()
            .map(|entry| {
                object([
                    ("name", entry["name"].clone()),
                    (
                        "sources",
                        Value::Array(
                            entry["sources"]
                                .as_array()
                                .expect("sources should be an array")
                                .iter()
                                .map(|source| {
                                    object([
                                        ("file", source["file"].clone()),
                                        ("lines", glossary_lines_to_array(&source["lines"])),
                                        ("used_by", source["used_by"].clone()),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect(),
    )
}

fn normalize_search_filter_json_entry(entry: &Value) -> Value {
    object([
        ("file", entry["file"].clone()),
        (
            "exports",
            entry
                .get("exports")
                .map(|exports| normalize_export_array(exports, false))
                .unwrap_or_else(|| json!([])),
        ),
        (
            "imports",
            entry.get("imports").cloned().unwrap_or_else(|| json!([])),
        ),
        (
            "dependencies",
            entry
                .get("dependencies")
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        ("loc", entry["loc"].clone()),
    ])
}

fn parse_search_filter_text(text: &str) -> Vec<Value> {
    let mut entries = Vec::new();
    let mut current: Option<Map<String, Value>> = None;

    let flush = |current: &mut Option<Map<String, Value>>, entries: &mut Vec<Value>| {
        if let Some(mut entry) = current.take() {
            for key in ["exports", "imports", "dependencies"] {
                entry
                    .entry(key.to_string())
                    .or_insert_with(|| Value::Array(Vec::new()));
            }
            entries.push(Value::Object(entry));
        }
    };

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with(" ") && current.is_none() {
            continue;
        }
        if !line.starts_with(' ') {
            if line.contains("file(s) found") {
                continue;
            }
            flush(&mut current, &mut entries);
            let mut entry = Map::new();
            entry.insert("file".to_string(), Value::String(line.to_string()));
            current = Some(entry);
        } else if let Some((key, value)) = line.trim().split_once(": ")
            && let Some(entry) = &mut current
        {
            match key {
                "exports" => {
                    entry.insert(
                        "exports".to_string(),
                        Value::Array(parse_compact_exports(value)),
                    );
                }
                "imports" | "dependencies" => {
                    entry.insert(key.to_string(), Value::Array(parse_csv_strings(value)));
                }
                "loc" => {
                    entry.insert("loc".to_string(), parse_atom(value));
                }
                _ => {}
            }
        }
    }

    flush(&mut current, &mut entries);
    entries
}

fn parse_ls_rows(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .filter_map(|row| {
            let (file_part, comment) = row.split_once('#')?;
            let file = file_part.trim().to_string();
            let mut loc = None;
            let mut exports = None;
            let mut downstream = Some(0usize);

            for part in comment.split(',').map(str::trim) {
                if let Some(value) = part.strip_prefix("loc: ") {
                    loc = value.parse::<usize>().ok();
                } else if let Some(value) = part.strip_prefix("exports: ") {
                    exports = value.parse::<usize>().ok();
                } else if let Some(value) = part.strip_suffix(" downstream") {
                    downstream = value
                        .split_whitespace()
                        .last()
                        .and_then(|n| n.parse::<usize>().ok());
                }
            }

            Some(object([
                ("file", Value::String(file)),
                ("loc", json!(loc?)),
                ("exports", json!(exports?)),
                ("downstream", json!(downstream?)),
            ]))
        })
        .collect()
}

fn parse_export_table(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return None;
            }
            let mut entry = Map::new();
            entry.insert("name".to_string(), Value::String(parts[0].to_string()));
            entry.insert("file".to_string(), Value::String(parts[1].to_string()));
            if parts.len() >= 4 {
                entry.insert("lines".to_string(), parse_lines_pair(&parts[2..].join(" ")));
            }
            Some(Value::Object(entry))
        })
        .collect()
}

fn parse_named_lines_block(text: &str, section: &str) -> Vec<Value> {
    let mut entries = Vec::new();
    let mut in_section = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim_end().to_string();
        if line.trim() == format!("{section}:") {
            in_section = true;
            continue;
        }
        if in_section {
            if line.starts_with("  ") && !line.starts_with("    ") {
                let item = line.trim();
                if let Some((name, value)) = item.split_once(": ") {
                    entries.push(object([
                        ("name", Value::String(name.to_string())),
                        ("lines", parse_lines_pair(value)),
                    ]));
                } else {
                    entries.push(object([("name", Value::String(item.to_string()))]));
                }
            } else if !line.starts_with(' ') && !line.trim().is_empty() {
                break;
            }
        }
    }
    entries
}

fn parse_list_block(text: &str, section: &str) -> Vec<Value> {
    let mut items = Vec::new();
    let mut in_section = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim_end().to_string();
        if line.trim() == format!("{section}:") {
            in_section = true;
            continue;
        }
        if in_section {
            if let Some(item) = line.trim().strip_prefix("- ") {
                if let Some(rest) = item.strip_prefix("file: ") {
                    let file = rest.split("  depth:").next().unwrap_or(rest).trim();
                    items.push(Value::String(file.to_string()));
                } else {
                    items.push(Value::String(item.trim().to_string()));
                }
            } else if !line.starts_with(' ') && !line.trim().is_empty() {
                break;
            }
        }
    }
    items
}

fn parse_import_hits(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() {
                return None;
            }
            let (package, files) = line.split_once("  (")?;
            let files = files.trim_end_matches(')');
            Some(object([
                ("package", Value::String(package.to_string())),
                ("files", Value::Array(parse_csv_strings(files))),
            ]))
        })
        .collect()
}

fn parse_named_import_hits(text: &str) -> Vec<Value> {
    let mut hits = Vec::new();
    let mut current: Option<Map<String, Value>> = None;
    for raw in text.lines() {
        if raw.starts_with("  ") && !raw.starts_with("    ") {
            if let Some(hit) = current.take() {
                hits.push(Value::Object(hit));
            }
            let line = raw.trim();
            let (symbol, source) = line.split_once(" from ").unwrap();
            let mut hit = Map::new();
            hit.insert("symbol".to_string(), Value::String(symbol.to_string()));
            hit.insert("source".to_string(), Value::String(source.to_string()));
            hit.insert("files".to_string(), Value::Array(Vec::new()));
            current = Some(hit);
        } else if raw.starts_with("    ")
            && let Some(hit) = &mut current
        {
            hit.get_mut("files")
                .unwrap()
                .as_array_mut()
                .unwrap()
                .push(Value::String(raw.trim().to_string()));
        }
    }
    if let Some(hit) = current {
        hits.push(Value::Object(hit));
    }
    hits
}

fn parse_glossary_source(value: &str) -> Map<String, Value> {
    let mut source = Map::new();
    if let Some((file, lines)) = value.rsplit_once(" [") {
        source.insert("file".to_string(), Value::String(file.to_string()));
        let lines = lines.trim_end_matches(']').replace('-', ", ");
        source.insert("lines".to_string(), parse_lines_pair(&format!("[{lines}]")));
    } else {
        source.insert("file".to_string(), Value::String(value.to_string()));
        source.insert("lines".to_string(), Value::Null);
    }
    source
}

fn search_sections(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut sections = std::collections::BTreeMap::new();
    let mut current_name: Option<String> = None;
    let mut current_lines = Vec::new();

    let flush = |name: &mut Option<String>,
                 lines: &mut Vec<String>,
                 sections: &mut std::collections::BTreeMap<String, String>| {
        if let Some(section_name) = name.take() {
            sections.insert(section_name, lines.join("\n"));
            lines.clear();
        }
    };

    for raw in text.lines() {
        let line = raw.trim_end();
        match line {
            "EXPORTS" | "FILES" | "IMPORTS" | "NAMED IMPORTS" => {
                flush(&mut current_name, &mut current_lines, &mut sections);
                current_name = Some(line.to_string());
            }
            _ if current_name.is_some() => current_lines.push(line.to_string()),
            _ => {}
        }
    }
    flush(&mut current_name, &mut current_lines, &mut sections);
    sections
}

fn normalize_export_array(value: &Value, include_file: bool) -> Value {
    Value::Array(
        value
            .as_array()
            .expect("exports should be an array")
            .iter()
            .map(|entry| {
                let mut object = Map::new();
                object.insert("name".to_string(), entry["name"].clone());
                if include_file {
                    object.insert("file".to_string(), entry["file"].clone());
                }
                if let Some(lines) = entry.get("lines") {
                    object.insert("lines".to_string(), lines.clone());
                }
                Value::Object(object)
            })
            .collect(),
    )
}

fn normalize_import_hits(value: &Value) -> Value {
    Value::Array(
        value
            .as_array()
            .expect("imports should be an array")
            .iter()
            .map(|entry| {
                object([
                    ("package", entry["package"].clone()),
                    ("files", entry["files"].clone()),
                ])
            })
            .collect(),
    )
}

fn normalize_named_import_hits(value: &Value) -> Value {
    Value::Array(
        value
            .as_array()
            .expect("named imports should be an array")
            .iter()
            .map(|entry| {
                object([
                    ("symbol", entry["symbol"].clone()),
                    ("source", entry["source"].clone()),
                    ("files", entry["files"].clone()),
                ])
            })
            .collect(),
    )
}

fn parse_compact_exports(value: &str) -> Vec<Value> {
    split_top_level_csv(value)
        .into_iter()
        .filter_map(|item| {
            if let Some((name, lines)) = item.rsplit_once(" [") {
                Some(object([
                    ("name", Value::String(name.trim().to_string())),
                    ("lines", parse_lines_pair(&format!("[{}", lines))),
                ]))
            } else if !item.trim().is_empty() {
                Some(object([("name", Value::String(item.trim().to_string()))]))
            } else {
                None
            }
        })
        .collect()
}

fn parse_inline_array(value: &str) -> Vec<Value> {
    value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .map(|inner| {
            if inner.trim().is_empty() {
                Vec::new()
            } else {
                split_top_level_csv(inner)
                    .into_iter()
                    .map(|item| parse_atom(item.trim()))
                    .collect()
            }
        })
        .unwrap_or_default()
}

fn parse_csv_strings(value: &str) -> Vec<Value> {
    split_top_level_csv(value)
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .map(|item| Value::String(item.trim().to_string()))
        .collect()
}

fn split_top_level_csv(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (idx, ch) in value.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(value[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    out.push(value[start..].trim().to_string());
    out
}

fn scalar_field(text: &str, key: &str) -> Value {
    for raw in text.lines() {
        let line = strip_comment(raw);
        if let Some(value) = line.trim().strip_prefix(&format!("{key}: ")) {
            return parse_atom(value.trim());
        }
    }
    panic!("missing field {key} in:\n{text}");
}

fn array_field(text: &str, key: &str) -> Option<Value> {
    for raw in text.lines() {
        let line = strip_comment(raw);
        if let Some(value) = line.trim().strip_prefix(&format!("{key}: ")) {
            return Some(Value::Array(parse_inline_array(value)));
        }
    }
    None
}

fn parse_lines_pair(value: &str) -> Value {
    let items = parse_inline_array(value);
    assert_eq!(items.len(), 2, "expected line pair, got {value}");
    Value::Array(items)
}

fn glossary_lines_to_array(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    json!([value["start"], value["end"]])
}

fn parse_atom(value: &str) -> Value {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(number) = value.parse::<usize>() {
        json!(number)
    } else {
        Value::String(value.to_string())
    }
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn sort_objects_by_file(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by(|a, b| {
        a.get("file")
            .and_then(Value::as_str)
            .cmp(&b.get("file").and_then(Value::as_str))
    });
    values
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}
