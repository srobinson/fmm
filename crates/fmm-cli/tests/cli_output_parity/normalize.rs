use serde_json::{Map, Value, json};

use super::Projection;
use super::parse::{
    array_field, object, parse_export_table, parse_glossary_source, parse_import_hits,
    parse_inline_array, parse_list_block, parse_ls_rows, parse_named_import_hits,
    parse_named_lines_block, parse_search_filter_text, scalar_field, search_sections, strip_ansi,
    strip_comment,
};

pub(super) fn normalize_text(projection: Projection, stdout: &[u8]) -> Value {
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

pub(super) fn normalize_json(projection: Projection, stdout: &[u8]) -> Value {
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

fn glossary_lines_to_array(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    json!([value["start"], value["end"]])
}

fn sort_objects_by_file(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by(|a, b| {
        a.get("file")
            .and_then(Value::as_str)
            .cmp(&b.get("file").and_then(Value::as_str))
    });
    values
}
