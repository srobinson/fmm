use regex::Regex;
use std::collections::HashSet;

use crate::manifest::Manifest;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CodeReference {
    FilePath {
        path: String,
        line: Option<u64>,
    },
    Symbol {
        name: String,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct ResolvedReference {
    pub file_path: String,
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
    pub match_reason: String,
}

const COMMON_WORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "not",
    "but",
    "are",
    "was",
    "has",
    "this",
    "that",
    "with",
    "from",
    "will",
    "can",
    "should",
    "would",
    "could",
    "may",
    "might",
    "must",
    "shall",
    "need",
    "use",
    "run",
    "get",
    "set",
    "add",
    "fix",
    "bug",
    "error",
    "issue",
    "test",
    "file",
    "true",
    "false",
    "null",
    "none",
    "nil",
    "undefined",
    "void",
    "return",
    "if",
    "else",
    "then",
    "when",
    "while",
    "loop",
    "break",
    "continue",
    "try",
    "catch",
    "throw",
    "npm",
    "yarn",
    "pnpm",
    "cargo",
    "pip",
    "brew",
    "apt",
    "git",
    "cd",
    "ls",
    "rm",
    "mkdir",
    "cat",
    "echo",
    "grep",
    "sed",
    "awk",
    "curl",
    "wget",
];

const PATH_PREFIXES: &[&str] = &[
    "src/",
    "lib/",
    "app/",
    "pkg/",
    "cmd/",
    "internal/",
    "test/",
    "tests/",
    "spec/",
    "config/",
    "scripts/",
    "bin/",
    "crate/",
    "packages/",
    "modules/",
];

const CODE_EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".js", ".jsx", ".py", ".rs", ".go", ".java", ".cpp", ".c", ".cs", ".rb", ".php",
    ".swift", ".kt", ".scala", ".ex", ".exs", ".hs", ".ml", ".vue", ".svelte", ".yaml", ".yml",
    ".json", ".toml", ".cfg", ".ini", ".xml", ".html", ".css", ".scss", ".less", ".md", ".txt",
    ".sh", ".bash", ".zsh",
];

pub fn extract_references(body: &str) -> Vec<CodeReference> {
    let mut refs: HashSet<CodeReference> = HashSet::new();

    extract_file_paths(body, &mut refs);
    extract_code_blocks(body, &mut refs);
    extract_symbols(body, &mut refs);

    refs.into_iter().collect()
}

fn extract_file_paths(body: &str, refs: &mut HashSet<CodeReference>) {
    // Backtick-wrapped paths with optional line numbers: `src/foo/bar.ts`, `hooks.js:3035`
    let backtick_re = Regex::new(r"`([^`\s]+\.[a-zA-Z]{1,10}(?::\d+|#L\d+)?)`").unwrap();
    for cap in backtick_re.captures_iter(body) {
        let path = &cap[1];
        if !path.starts_with("http") {
            let (clean_path, line) = parse_line_number(path);
            if looks_like_file_path(&clean_path) {
                refs.insert(CodeReference::FilePath {
                    path: clean_path,
                    line,
                });
            }
        }
    }

    // Bare paths with line numbers: file.ts:123 or file.ts#L123
    let line_ref_re = Regex::new(r"(?:^|\s)(\S+\.[a-zA-Z]{1,10})(?::(\d+)|#L(\d+))").unwrap();
    for cap in line_ref_re.captures_iter(body) {
        let path = cap[1].to_string();
        if looks_like_file_path(&path) && !path.starts_with("http") {
            let line = cap
                .get(2)
                .or_else(|| cap.get(3))
                .and_then(|m| m.as_str().parse().ok());
            refs.insert(CodeReference::FilePath { path, line });
        }
    }

    // Bare paths with known prefixes
    let bare_path_re = Regex::new(r"(?:^|\s)((?:src|lib|app|pkg|cmd|internal|test|tests|spec|config|scripts|bin|packages|modules)/\S+\.[a-zA-Z]{1,10})").unwrap();
    for cap in bare_path_re.captures_iter(body) {
        let path = cap[1].to_string();
        if !path.starts_with("http") {
            let (clean_path, line) = parse_line_number(&path);
            refs.insert(CodeReference::FilePath {
                path: clean_path,
                line,
            });
        }
    }
}

fn extract_symbols(body: &str, refs: &mut HashSet<CodeReference>) {
    // Backtick-wrapped identifiers: `getADRStatus()`, `UserService`
    let symbol_re = Regex::new(r"`([a-zA-Z_][a-zA-Z0-9_]*(?:\(\))?)`").unwrap();
    for cap in symbol_re.captures_iter(body) {
        let mut name = cap[1].to_string();

        // Skip if it's a file path (contains / or has extension)
        if name.contains('/') || looks_like_file_path(&name) {
            continue;
        }

        // Strip trailing ()
        if name.ends_with("()") {
            name = name[..name.len() - 2].to_string();
        }

        // Skip common English/shell words
        if COMMON_WORDS.contains(&name.to_lowercase().as_str()) {
            continue;
        }

        // Skip very short identifiers (likely not meaningful)
        if name.len() < 3 {
            continue;
        }

        refs.insert(CodeReference::Symbol { name });
    }
}

fn extract_code_blocks(body: &str, refs: &mut HashSet<CodeReference>) {
    let block_re = Regex::new(r"```(\w*)\n([\s\S]*?)```").unwrap();
    let ident_re = Regex::new(r"\b(?:fn|function|class|interface|type|struct|enum|trait|def|const|let|var)\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    for cap in block_re.captures_iter(body) {
        let language = if cap[1].is_empty() {
            None
        } else {
            Some(cap[1].to_string())
        };
        let content = cap[2].to_string();

        for ident_cap in ident_re.captures_iter(&content) {
            let name = ident_cap[1].to_string();
            if name.len() >= 3 {
                refs.insert(CodeReference::Symbol { name });
            }
        }

        refs.insert(CodeReference::CodeBlock { language, content });
    }
}

fn looks_like_file_path(s: &str) -> bool {
    // Has a path separator
    if s.contains('/') {
        return CODE_EXTENSIONS.iter().any(|ext| s.ends_with(ext))
            || PATH_PREFIXES.iter().any(|p| s.starts_with(p));
    }
    // filename.ext pattern
    if let Some(dot_pos) = s.rfind('.') {
        let ext = &s[dot_pos..];
        return CODE_EXTENSIONS.contains(&ext);
    }
    false
}

fn parse_line_number(path: &str) -> (String, Option<u64>) {
    // file.ts:123
    if let Some(colon_pos) = path.rfind(':') {
        if let Ok(line) = path[colon_pos + 1..].parse::<u64>() {
            return (path[..colon_pos].to_string(), Some(line));
        }
    }
    // file.ts#L123
    if let Some(hash_pos) = path.rfind("#L") {
        if let Ok(line) = path[hash_pos + 2..].parse::<u64>() {
            return (path[..hash_pos].to_string(), Some(line));
        }
    }
    (path.to_string(), None)
}

const MAX_RESOLVED_FILES: usize = 20;

pub fn resolve_references(
    refs: &[CodeReference],
    manifest: &Manifest,
) -> (Vec<ResolvedReference>, Vec<String>) {
    let mut resolved: Vec<ResolvedReference> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    // Phase 1: Direct matches
    let code_block_ident_re =
        Regex::new(r"\b([A-Z][a-zA-Z0-9_]+|[a-z][a-zA-Z0-9_]*(?:_[a-z][a-zA-Z0-9_]*))\b").unwrap();
    for r in refs {
        match r {
            CodeReference::FilePath { path, .. } => {
                if let Some(resolved_ref) = resolve_file_path(path, manifest) {
                    if seen_paths.insert(resolved_ref.file_path.clone()) {
                        resolved.push(resolved_ref);
                    }
                } else {
                    unresolved.push(format!("file: {}", path));
                }
            }
            CodeReference::Symbol { name } => {
                if let Some(file_path) = manifest.export_index.get(name) {
                    if seen_paths.insert(file_path.clone()) {
                        if let Some(entry) = manifest.files.get(file_path) {
                            resolved.push(ResolvedReference {
                                file_path: file_path.clone(),
                                exports: entry.exports.clone(),
                                imports: entry.imports.clone(),
                                dependencies: entry.dependencies.clone(),
                                loc: entry.loc,
                                match_reason: format!("export match: {}", name),
                            });
                        }
                    }
                } else {
                    unresolved.push(format!("symbol: {}", name));
                }
            }
            CodeReference::CodeBlock { content, .. } => {
                for cap in code_block_ident_re.captures_iter(content) {
                    let name = &cap[1];
                    if let Some(file_path) = manifest.export_index.get(name) {
                        if seen_paths.insert(file_path.clone()) {
                            if let Some(entry) = manifest.files.get(file_path) {
                                resolved.push(ResolvedReference {
                                    file_path: file_path.clone(),
                                    exports: entry.exports.clone(),
                                    imports: entry.imports.clone(),
                                    dependencies: entry.dependencies.clone(),
                                    loc: entry.loc,
                                    match_reason: format!("code block symbol: {}", name),
                                });
                            }
                        }
                    }
                }
            }
        }

        if resolved.len() >= MAX_RESOLVED_FILES {
            break;
        }
    }

    // Phase 2: Dependency fan-out (one hop)
    if resolved.len() < MAX_RESOLVED_FILES {
        let mut fanout: Vec<ResolvedReference> = Vec::new();

        let current_paths: Vec<String> = resolved.iter().map(|r| r.file_path.clone()).collect();
        for path in &current_paths {
            if let Some(entry) = manifest.files.get(path) {
                // Upstream deps
                for dep in &entry.dependencies {
                    if seen_paths.contains(dep) {
                        continue;
                    }
                    if let Some(dep_entry) = manifest.files.get(dep) {
                        if seen_paths.insert(dep.clone()) {
                            fanout.push(ResolvedReference {
                                file_path: dep.clone(),
                                exports: dep_entry.exports.clone(),
                                imports: dep_entry.imports.clone(),
                                dependencies: dep_entry.dependencies.clone(),
                                loc: dep_entry.loc,
                                match_reason: format!("dependency of {}", path),
                            });
                        }
                    }
                }

                // Downstream dependents
                for (other_path, other_entry) in &manifest.files {
                    if seen_paths.contains(other_path) {
                        continue;
                    }
                    if other_entry.dependencies.iter().any(|d| d == path)
                        && seen_paths.insert(other_path.clone())
                    {
                        fanout.push(ResolvedReference {
                            file_path: other_path.clone(),
                            exports: other_entry.exports.clone(),
                            imports: other_entry.imports.clone(),
                            dependencies: other_entry.dependencies.clone(),
                            loc: other_entry.loc,
                            match_reason: format!("depends on {}", path),
                        });
                    }
                }
            }

            if resolved.len() + fanout.len() >= MAX_RESOLVED_FILES {
                break;
            }
        }

        resolved.extend(fanout);
    }

    // Phase 3: Fallback — entry point files if nothing resolved
    if resolved.is_empty() {
        let entry_points = [
            "index.ts", "index.js", "main.rs", "lib.rs", "mod.rs", "main.py", "app.py", "main.go",
            "App.tsx", "App.jsx",
        ];

        for (path, entry) in &manifest.files {
            let filename = path.rsplit('/').next().unwrap_or(path);
            if entry_points.contains(&filename) && seen_paths.insert(path.clone()) {
                resolved.push(ResolvedReference {
                    file_path: path.clone(),
                    exports: entry.exports.clone(),
                    imports: entry.imports.clone(),
                    dependencies: entry.dependencies.clone(),
                    loc: entry.loc,
                    match_reason: "entry point (fallback)".to_string(),
                });
            }

            if resolved.len() >= MAX_RESOLVED_FILES {
                break;
            }
        }
    }

    // Cap at limit
    resolved.truncate(MAX_RESOLVED_FILES);

    (resolved, unresolved)
}

fn resolve_file_path(path: &str, manifest: &Manifest) -> Option<ResolvedReference> {
    // Exact match
    if let Some(entry) = manifest.files.get(path) {
        return Some(ResolvedReference {
            file_path: path.to_string(),
            exports: entry.exports.clone(),
            imports: entry.imports.clone(),
            dependencies: entry.dependencies.clone(),
            loc: entry.loc,
            match_reason: "exact file path".to_string(),
        });
    }

    // Suffix match: find manifest keys that end with the given path
    for (manifest_path, entry) in &manifest.files {
        if manifest_path.ends_with(path)
            && (manifest_path.len() == path.len()
                || manifest_path.as_bytes()[manifest_path.len() - path.len() - 1] == b'/')
        {
            return Some(ResolvedReference {
                file_path: manifest_path.clone(),
                exports: entry.exports.clone(),
                imports: entry.imports.clone(),
                dependencies: entry.dependencies.clone(),
                loc: entry.loc,
                match_reason: format!("suffix match for {}", path),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::FileEntry;
    use std::collections::HashMap;

    #[test]
    fn extract_backtick_file_paths() {
        let body = "Check `src/auth/login.ts` and `lib/utils.py` for the bug.";
        let refs = extract_references(body);
        let paths: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::FilePath { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert!(paths.contains(&"src/auth/login.ts"));
        assert!(paths.contains(&"lib/utils.py"));
    }

    #[test]
    fn extract_paths_with_line_numbers() {
        let body = "The error is at `hooks.js:3035` and also hooks.js#L100";
        let refs = extract_references(body);
        let file_refs: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::FilePath { path, line } => Some((path.as_str(), *line)),
                _ => None,
            })
            .collect();
        assert!(file_refs.contains(&("hooks.js", Some(3035))));
        assert!(file_refs.contains(&("hooks.js", Some(100))));
    }

    #[test]
    fn extract_symbols_from_backticks() {
        let body = "The function `getADRStatus()` in `UserService` is broken.";
        let refs = extract_references(body);
        let symbols: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::Symbol { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(symbols.contains(&"getADRStatus"));
        assert!(symbols.contains(&"UserService"));
    }

    #[test]
    fn extract_code_blocks() {
        let body = "Here's the fix:\n```typescript\nfunction handleError(err: Error) {\n  console.log(err);\n}\n```";
        let refs = extract_references(body);
        let has_block = refs
            .iter()
            .any(|r| matches!(r, CodeReference::CodeBlock { .. }));
        assert!(has_block);
        // Should also extract handleError as a symbol from the block
        let symbols: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::Symbol { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(symbols.contains(&"handleError"));
    }

    #[test]
    fn skip_urls_and_common_words() {
        let body = "`true` `false` `null` `npm` should not be extracted as symbols.";
        let refs = extract_references(body);
        let symbols: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::Symbol { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(symbols.is_empty());
    }

    #[test]
    fn deduplicates_references() {
        let body = "See `src/main.rs` and also `src/main.rs` again.";
        let refs = extract_references(body);
        let paths: Vec<_> = refs
            .iter()
            .filter_map(|r| match r {
                CodeReference::FilePath { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 1);
    }

    fn mock_manifest() -> Manifest {
        let mut files = HashMap::new();
        files.insert(
            "src/auth/login.ts".to_string(),
            FileEntry {
                exports: vec!["login".to_string(), "logout".to_string()],
                imports: vec!["express".to_string()],
                dependencies: vec!["src/auth/utils.ts".to_string()],
                loc: 150,
            },
        );
        files.insert(
            "src/auth/utils.ts".to_string(),
            FileEntry {
                exports: vec!["hashPassword".to_string()],
                imports: vec!["bcrypt".to_string()],
                dependencies: vec![],
                loc: 40,
            },
        );
        files.insert(
            "src/index.ts".to_string(),
            FileEntry {
                exports: vec!["app".to_string()],
                imports: vec!["express".to_string()],
                dependencies: vec!["src/auth/login.ts".to_string()],
                loc: 30,
            },
        );

        let mut export_index = HashMap::new();
        export_index.insert("login".to_string(), "src/auth/login.ts".to_string());
        export_index.insert("logout".to_string(), "src/auth/login.ts".to_string());
        export_index.insert("hashPassword".to_string(), "src/auth/utils.ts".to_string());
        export_index.insert("app".to_string(), "src/index.ts".to_string());

        Manifest {
            version: "2.0".to_string(),
            generated: chrono::Utc::now(),
            files,
            export_index,
        }
    }

    #[test]
    fn resolve_exact_file_path() {
        let manifest = mock_manifest();
        let refs = vec![CodeReference::FilePath {
            path: "src/auth/login.ts".to_string(),
            line: None,
        }];
        let (resolved, unresolved) = resolve_references(&refs, &manifest);
        // First result is the direct match; fan-out adds neighbors
        assert!(!resolved.is_empty());
        assert_eq!(resolved[0].file_path, "src/auth/login.ts");
        assert_eq!(resolved[0].match_reason, "exact file path");
        assert!(unresolved.is_empty());
    }

    #[test]
    fn resolve_suffix_match() {
        let manifest = mock_manifest();
        let refs = vec![CodeReference::FilePath {
            path: "auth/login.ts".to_string(),
            line: None,
        }];
        let (resolved, _) = resolve_references(&refs, &manifest);
        assert!(!resolved.is_empty());
        assert!(resolved[0].match_reason.contains("suffix match"));
    }

    #[test]
    fn resolve_symbol_via_export_index() {
        let manifest = mock_manifest();
        let refs = vec![CodeReference::Symbol {
            name: "hashPassword".to_string(),
        }];
        let (resolved, unresolved) = resolve_references(&refs, &manifest);
        // Direct match + potential fan-out of dependents
        assert!(!resolved.is_empty());
        assert_eq!(resolved[0].file_path, "src/auth/utils.ts");
        assert!(resolved[0].match_reason.contains("export match"));
        assert!(unresolved.is_empty());
    }

    #[test]
    fn dependency_fanout_adds_neighbors() {
        let manifest = mock_manifest();
        let refs = vec![CodeReference::FilePath {
            path: "src/auth/login.ts".to_string(),
            line: None,
        }];
        let (resolved, _) = resolve_references(&refs, &manifest);
        // Should include login.ts + its upstream dep (utils.ts) + downstream (index.ts)
        let paths: Vec<&str> = resolved.iter().map(|r| r.file_path.as_str()).collect();
        assert!(paths.contains(&"src/auth/login.ts"));
        assert!(paths.contains(&"src/auth/utils.ts")); // upstream dep
        assert!(paths.contains(&"src/index.ts")); // depends on login.ts
    }

    #[test]
    fn fallback_to_entry_points() {
        let manifest = mock_manifest();
        let refs = vec![CodeReference::Symbol {
            name: "nonexistent".to_string(),
        }];
        let (resolved, _) = resolve_references(&refs, &manifest);
        // Should fall back to index.ts
        let paths: Vec<&str> = resolved.iter().map(|r| r.file_path.as_str()).collect();
        assert!(paths.contains(&"src/index.ts"));
    }

    #[test]
    fn cap_at_20_files() {
        let mut files = HashMap::new();
        let mut export_index = HashMap::new();
        for i in 0..30 {
            let path = format!("src/file_{}.ts", i);
            let export = format!("export_{}", i);
            files.insert(
                path.clone(),
                FileEntry {
                    exports: vec![export.clone()],
                    imports: vec![],
                    dependencies: vec![],
                    loc: 10,
                },
            );
            export_index.insert(export, path);
        }

        let manifest = Manifest {
            version: "2.0".to_string(),
            generated: chrono::Utc::now(),
            files,
            export_index,
        };

        let refs: Vec<CodeReference> = (0..30)
            .map(|i| CodeReference::Symbol {
                name: format!("export_{}", i),
            })
            .collect();

        let (resolved, _) = resolve_references(&refs, &manifest);
        assert!(resolved.len() <= 20);
    }
}
