use serde_json::{Map, Value, json};

pub(super) fn parse_search_filter_text(text: &str) -> Vec<Value> {
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

pub(super) fn parse_ls_rows(text: &str) -> Vec<Value> {
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

pub(super) fn parse_export_table(text: &str) -> Vec<Value> {
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

pub(super) fn parse_named_lines_block(text: &str, section: &str) -> Vec<Value> {
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

pub(super) fn parse_list_block(text: &str, section: &str) -> Vec<Value> {
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

pub(super) fn parse_import_hits(text: &str) -> Vec<Value> {
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

pub(super) fn parse_named_import_hits(text: &str) -> Vec<Value> {
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

pub(super) fn parse_glossary_source(value: &str) -> Map<String, Value> {
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

pub(super) fn search_sections(text: &str) -> std::collections::BTreeMap<String, String> {
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

pub(super) fn parse_inline_array(value: &str) -> Vec<Value> {
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

pub(super) fn scalar_field(text: &str, key: &str) -> Value {
    for raw in text.lines() {
        let line = strip_comment(raw);
        if let Some(value) = line.trim().strip_prefix(&format!("{key}: ")) {
            return parse_atom(value.trim());
        }
    }
    panic!("missing field {key} in:\n{text}");
}

pub(super) fn array_field(text: &str, key: &str) -> Option<Value> {
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

fn parse_atom(value: &str) -> Value {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(number) = value.parse::<usize>() {
        json!(number)
    } else {
        Value::String(value.to_string())
    }
}

pub(super) fn strip_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before, _)| before)
        .unwrap_or(line)
}

pub(super) fn strip_ansi(input: &str) -> String {
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

pub(super) fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}
