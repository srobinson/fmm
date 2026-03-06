use std::collections::HashMap;

use serde::Deserialize;

use super::{ExportLines, FileEntry};

/// Typed representation of a `.fmm` sidecar file for serde_yaml deserialization.
/// v0.4: exports as map with line ranges, named_imports and namespace_imports top-level.
#[derive(Debug, Deserialize)]
struct SidecarData {
    file: String,
    #[serde(default)]
    exports: Option<serde_yaml::Value>,
    /// Flat map of `ClassName.method: [start, end]` entries.
    #[serde(default)]
    methods: Option<serde_yaml::Value>,
    #[serde(default)]
    imports: Option<Vec<String>>,
    #[serde(default)]
    dependencies: Option<Vec<String>>,
    #[serde(default)]
    loc: Option<usize>,
    /// Named imports per source module: path → [original_name, ...].
    #[serde(default)]
    named_imports: Option<serde_yaml::Value>,
    /// Source paths of namespace/wildcard imports.
    #[serde(default)]
    namespace_imports: Option<Vec<String>>,
    /// Captures all other fields (fmm version, modified, language-specific sections)
    #[serde(flatten)]
    _extra: HashMap<String, serde_yaml::Value>,
}

/// Remove duplicate mapping keys from a YAML document by keeping the first occurrence of each
/// key at each indentation level. Used to recover from sidecars generated before the method-
/// overload dedup fix, where TypeScript overloads produced duplicate keys in `methods:` that
/// serde_yaml 0.9 rejects.
fn dedup_yaml_mapping_keys(content: &str) -> String {
    let mut seen: std::collections::HashSet<(usize, String)> = std::collections::HashSet::new();
    let mut result: Vec<&str> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        // Identify mapping keys: non-empty text before ':', no spaces in key, not a
        // comment, list item, or flow-sequence/mapping start.
        if !trimmed.starts_with('#')
            && !trimmed.starts_with('-')
            && !trimmed.starts_with('[')
            && !trimmed.starts_with('{')
        {
            if let Some(colon_pos) = trimmed.find(':') {
                let key = &trimmed[..colon_pos];
                if !key.is_empty() && !key.contains(' ') && !seen.insert((indent, key.to_string()))
                {
                    continue; // duplicate — skip this line
                }
            }
        }
        result.push(line);
    }
    result.join("\n")
}

/// Parse a sidecar YAML file into (file_path, FileEntry).
/// Parses a v0.4 sidecar file into a `(file_path, FileEntry)` pair.
/// If serde_yaml rejects the document due to duplicate mapping keys (TypeScript overloads in
/// sidecars generated before the dedup fix), deduplicates and retries with a warning.
pub(super) fn parse_sidecar(content: &str) -> Option<(String, FileEntry)> {
    let data: SidecarData = match serde_yaml::from_str(content) {
        Ok(d) => d,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate entry") {
                eprintln!(
                    "fmm: warning: sidecar has duplicate YAML keys (TypeScript overloads) — \
                     deduplicating and reloading. Run 'fmm generate' to regenerate clean sidecars. \
                     Error: {}",
                    e
                );
                let deduped = dedup_yaml_mapping_keys(content);
                match serde_yaml::from_str(&deduped) {
                    Ok(d) => d,
                    Err(e2) => {
                        eprintln!("fmm: warning: sidecar still invalid after dedup: {}", e2);
                        return None;
                    }
                }
            } else {
                eprintln!("fmm: warning: failed to parse sidecar: {}", e);
                return None;
            }
        }
    };

    if data.file.is_empty() {
        return None;
    }

    let (exports, export_lines) = match data.exports {
        Some(serde_yaml::Value::Mapping(map)) => {
            // v0.4 format: exports:\n  foo: [1, 10]\n  bar: [12, 25]
            let mut names = Vec::new();
            let mut lines = Vec::new();
            for (key, value) in map {
                if let serde_yaml::Value::String(name) = key {
                    names.push(name);
                    match value {
                        serde_yaml::Value::Sequence(seq) if seq.len() == 2 => {
                            let start = seq[0].as_u64().unwrap_or(0) as usize;
                            let end = seq[1].as_u64().unwrap_or(0) as usize;
                            lines.push(ExportLines { start, end });
                        }
                        _ => {
                            lines.push(ExportLines { start: 0, end: 0 });
                        }
                    }
                }
            }
            let has_lines = lines.iter().any(|l| l.start > 0);
            (names, if has_lines { Some(lines) } else { None })
        }
        _ => (Vec::new(), None),
    };

    // Parse methods: section — flat map of "ClassName.method": [start, end]
    let methods = match data.methods {
        Some(serde_yaml::Value::Mapping(map)) => {
            let mut m = HashMap::new();
            for (key, value) in map {
                if let serde_yaml::Value::String(name) = key {
                    match value {
                        serde_yaml::Value::Sequence(seq) if seq.len() == 2 => {
                            let start = seq[0].as_u64().unwrap_or(0) as usize;
                            let end = seq[1].as_u64().unwrap_or(0) as usize;
                            m.insert(name, ExportLines { start, end });
                        }
                        _ => {}
                    }
                }
            }
            if m.is_empty() {
                None
            } else {
                Some(m)
            }
        }
        _ => None,
    };

    let modified = data
        ._extra
        .get("modified")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // ALP-862: extract function_names from typescript.function_names sidecar section.
    let function_names: Vec<String> = data
        ._extra
        .get("typescript")
        .and_then(|v| {
            if let serde_yaml::Value::Mapping(m) = v {
                m.get(serde_yaml::Value::String("function_names".to_string()))
            } else {
                None
            }
        })
        .and_then(|v| {
            if let serde_yaml::Value::Sequence(seq) = v {
                Some(
                    seq.iter()
                        .filter_map(|item| {
                            if let serde_yaml::Value::String(s) = item {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Parse named_imports: mapping of source path → [name, ...]
    let named_imports: HashMap<String, Vec<String>> = match data.named_imports {
        Some(serde_yaml::Value::Mapping(map)) => {
            let mut result = HashMap::new();
            for (key, value) in map {
                if let serde_yaml::Value::String(path) = key {
                    let names: Vec<String> = match value {
                        serde_yaml::Value::Sequence(seq) => seq
                            .into_iter()
                            .filter_map(|v| {
                                if let serde_yaml::Value::String(s) = v {
                                    Some(s)
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                    if !names.is_empty() {
                        result.insert(path, names);
                    }
                }
            }
            result
        }
        _ => HashMap::new(),
    };

    Some((
        data.file,
        FileEntry {
            exports,
            export_lines,
            methods,
            imports: data.imports.unwrap_or_default(),
            dependencies: data.dependencies.unwrap_or_default(),
            loc: data.loc.unwrap_or(0),
            modified,
            function_names,
            named_imports,
            namespace_imports: data.namespace_imports.unwrap_or_default(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ExportLines;

    #[test]
    fn test_parse_sidecar_named_imports() {
        let content = "file: src/app.ts\nfmm: v0.4\nexports:\n  App: [1, 20]\nloc: 20\nnamed_imports:\n  ./ReactFiberWorkLoop:\n    - scheduleUpdateOnFiber\n    - requestUpdateLane\n  ./ReactFiberLane:\n    - NoLane\nnamespace_imports:\n  - ./SomeModule\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/app.ts");
        let ni = &entry.named_imports;
        assert_eq!(
            ni["./ReactFiberWorkLoop"],
            vec!["scheduleUpdateOnFiber", "requestUpdateLane"]
        );
        assert_eq!(ni["./ReactFiberLane"], vec!["NoLane"]);
        assert_eq!(entry.namespace_imports, vec!["./SomeModule"]);
    }

    #[test]
    fn test_parse_sidecar_v03() {
        let content = "file: src/auth/session.ts\nfmm: v0.3\nexports:\n  createSession: [5, 20]\n  validateSession: [22, 45]\nimports: [jwt, redis]\ndependencies: [./types, ./config]\nloc: 234\nmodified: 2026-01-30";

        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/auth/session.ts");
        assert_eq!(entry.exports, vec!["createSession", "validateSession"]);
        let lines = entry.export_lines.unwrap();
        assert_eq!(lines[0], ExportLines { start: 5, end: 20 });
        assert_eq!(lines[1], ExportLines { start: 22, end: 45 });
    }

    #[test]
    fn test_parse_sidecar_methods_section() {
        let content = "file: src/factory.ts\nexports:\n  NestFactoryStatic: [43, 381]\nmethods:\n  NestFactoryStatic.create: [55, 89]\n  NestFactoryStatic.createApplicationContext: [132, 158]\nloc: 400\nmodified: 2026-03-05";

        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/factory.ts");
        assert_eq!(entry.exports, vec!["NestFactoryStatic"]);
        assert!(entry.export_lines.is_some());

        let methods = entry.methods.unwrap();
        assert_eq!(methods.len(), 2);
        let create = methods.get("NestFactoryStatic.create").unwrap();
        assert_eq!(create.start, 55);
        assert_eq!(create.end, 89);
        let ctx = methods
            .get("NestFactoryStatic.createApplicationContext")
            .unwrap();
        assert_eq!(ctx.start, 132);
        assert_eq!(ctx.end, 158);
    }

    #[test]
    fn test_parse_sidecar_no_methods_is_backward_compat() {
        let content = "file: src/auth/session.ts\nexports:\n  createSession: [5, 20]\nloc: 50\nmodified: 2026-03-05";
        let (_, entry) = parse_sidecar(content).unwrap();
        assert!(entry.methods.is_none());
    }

    #[test]
    fn test_file_entry_from_metadata_separates_methods() {
        use crate::parser::{ExportEntry, Metadata};
        let metadata = Metadata {
            exports: vec![
                ExportEntry::new("MyClass".to_string(), 10, 100),
                ExportEntry::method("doThing".to_string(), 20, 30, "MyClass".to_string()),
                ExportEntry::method("doOther".to_string(), 32, 45, "MyClass".to_string()),
            ],
            imports: vec![],
            dependencies: vec![],
            loc: 100,
            ..Default::default()
        };
        let fe = crate::manifest::FileEntry::from(metadata);
        assert_eq!(fe.exports, vec!["MyClass"]);
        let methods = fe.methods.unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods["MyClass.doThing"].start, 20);
        assert_eq!(methods["MyClass.doOther"].end, 45);
    }

    #[test]
    fn test_parse_sidecar_empty() {
        assert!(parse_sidecar("").is_none());
        assert!(parse_sidecar("loc: 10").is_none());
    }

    #[test]
    fn test_parse_sidecar_empty_exports() {
        let content = "file: src/empty.ts\nexports: []\nloc: 5\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/empty.ts");
        assert!(entry.exports.is_empty());
        assert_eq!(entry.loc, 5);
    }

    #[test]
    fn test_parse_sidecar_missing_optional_fields() {
        let content = "file: src/minimal.ts\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/minimal.ts");
        assert!(entry.exports.is_empty());
        assert!(entry.imports.is_empty());
        assert!(entry.dependencies.is_empty());
        assert_eq!(entry.loc, 0);
    }

    #[test]
    fn test_parse_sidecar_extra_fields() {
        let content = "file: src/lib.rs\nfmm: v0.3\nexports:\n  MyStruct: [5, 15]\nloc: 50\nrust:\n  derives: [Clone, Debug]\n";
        let (path, entry) = parse_sidecar(content).unwrap();
        assert_eq!(path, "src/lib.rs");
        assert_eq!(entry.exports, vec!["MyStruct"]);
        assert_eq!(entry.loc, 50);
        let lines = entry.export_lines.unwrap();
        assert_eq!(lines[0], ExportLines { start: 5, end: 15 });
    }

    #[test]
    fn parse_sidecar_recovers_from_duplicate_method_keys() {
        // TypeScript method overloads produce duplicate keys in methods: sections.
        // Sidecars generated before the dedup fix contain these and must still load.
        let content = concat!(
            "---\n",
            "file: packages/core/injector/module.ts\n",
            "fmm: v0.3+0.1.19\n",
            "exports:\n",
            "  Module: [44, 680]\n",
            "methods:\n",
            "  Module.token: [83, 85]\n",
            "  Module.token: [87, 89]\n",
            "  Module.isGlobal: [95, 97]\n",
            "  Module.isGlobal: [99, 101]\n",
            "imports: ['@nestjs/common/constants']\n",
            "dependencies: [./instance-wrapper, ./module-ref]\n",
            "loc: 680\n",
            "modified: 2026-03-05\n",
        );
        let result = parse_sidecar(content);
        assert!(
            result.is_some(),
            "parse_sidecar returned None for sidecar with duplicate method keys"
        );
        let (path, entry) = result.unwrap();
        assert_eq!(path, "packages/core/injector/module.ts");
        assert_eq!(entry.loc, 680);
        // Only one Module.token entry should survive dedup (first occurrence kept)
        assert!(entry
            .methods
            .as_ref()
            .is_some_and(|m| m.contains_key("Module.token")));
    }
}
