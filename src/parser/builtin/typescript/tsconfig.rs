use std::collections::HashMap;
use std::path::Path;

/// Walk up from `file_path` looking for `tsconfig.json`. When found, extract
/// `compilerOptions.paths` as a map of alias pattern → list of target templates.
/// Follows `extends` one level deep to pick up a base config's `paths`.
/// Returns an empty map when no tsconfig is found or no paths are configured.
pub(super) fn load_tsconfig_paths(file_path: &Path) -> HashMap<String, Vec<String>> {
    let mut dir = file_path.parent();
    while let Some(d) = dir {
        let tsconfig = d.join("tsconfig.json");
        if tsconfig.exists() {
            return read_tsconfig_paths(&tsconfig);
        }
        dir = d.parent();
    }
    HashMap::new()
}

/// Read `compilerOptions.paths` from a tsconfig file. Follows `extends` one
/// level deep so that a base config's paths are included.
pub(super) fn read_tsconfig_paths(tsconfig: &Path) -> HashMap<String, Vec<String>> {
    let Ok(content) = std::fs::read_to_string(tsconfig) else {
        return HashMap::new();
    };
    // tsconfig.json may contain comments — use serde_json with a best-effort
    // approach by stripping single-line comments first.
    let stripped = strip_json_comments(&content);
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&stripped) else {
        return HashMap::new();
    };

    let mut paths: HashMap<String, Vec<String>> = HashMap::new();

    // Follow `extends` one level deep to pick up base config paths.
    if let Some(extends) = json.get("extends").and_then(|v| v.as_str()) {
        let base_path = tsconfig.parent().unwrap_or(Path::new(".")).join(extends);
        let base = read_tsconfig_paths(&base_path);
        paths.extend(base);
    }

    // Own paths override base.
    if let Some(own_paths) = json
        .get("compilerOptions")
        .and_then(|o| o.get("paths"))
        .and_then(|p| p.as_object())
    {
        for (alias, targets) in own_paths {
            if let Some(target_arr) = targets.as_array() {
                let target_strings: Vec<String> = target_arr
                    .iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect();
                if !target_strings.is_empty() {
                    paths.insert(alias.clone(), target_strings);
                }
            }
        }
    }

    paths
}

/// Strip single-line `//` comments from JSON-like content so that tsconfig
/// files with comments can be parsed by serde_json.
pub(crate) fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                // Escaped character — include next char verbatim.
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            // Single-line comment — skip to end of line.
            for ch in chars.by_ref() {
                if ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Attempt to resolve a TypeScript path alias import to its physical path.
///
/// Given an import string like `@/utils/helper` and an aliases map like
/// `{"@/*": ["src/*"]}`, returns `Some("src/utils/helper")`. Returns `None`
/// when no alias matches.
pub(super) fn resolve_alias(
    import: &str,
    aliases: &HashMap<String, Vec<String>>,
) -> Option<String> {
    for (pattern, targets) in aliases {
        if let Some(resolved) = match_alias(import, pattern, targets) {
            return Some(resolved);
        }
    }
    None
}

/// Try a single alias pattern against the import. Patterns ending with `*`
/// act as prefix matches; exact patterns must match the full import string.
pub(crate) fn match_alias(import: &str, pattern: &str, targets: &[String]) -> Option<String> {
    let target = targets.first()?; // Use first target mapping.
    if let Some(prefix) = pattern.strip_suffix('*') {
        // Wildcard: `@/*` matches `@/foo/bar`, captures `foo/bar`.
        if let Some(rest) = import.strip_prefix(prefix) {
            if let Some(target_prefix) = target.strip_suffix('*') {
                return Some(format!("{}{}", target_prefix, rest));
            }
            // Target has no wildcard — map the whole import to the target.
            return Some(target.clone());
        }
    } else if import == pattern {
        // Exact match — map to first target (strip trailing `/*` if present).
        let mapped = target.strip_suffix("/*").unwrap_or(target).to_string();
        return Some(mapped);
    }
    None
}
