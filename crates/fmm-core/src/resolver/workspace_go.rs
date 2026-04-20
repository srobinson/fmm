use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{WorkspaceDiscoverer, WorkspaceEcosystem, WorkspaceInfo};

/// Go workspace discoverer. Reads root `go.work` `use` directives or falls
/// back to a single root `go.mod`.
pub struct GoWorkspaceDiscoverer;

impl WorkspaceDiscoverer for GoWorkspaceDiscoverer {
    fn ecosystem(&self) -> WorkspaceEcosystem {
        WorkspaceEcosystem::Go
    }

    fn detect(&self, repo_root: &Path) -> bool {
        repo_root.join("go.work").exists() || repo_root.join("go.mod").exists()
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let go_work = repo_root.join("go.work");
        if go_work.exists() {
            return discover_go_work(repo_root, &go_work);
        }

        discover_single_go_module(repo_root)
    }
}

fn discover_go_work(repo_root: &Path, go_work: &Path) -> WorkspaceInfo {
    let mut roots = Vec::new();
    let mut packages = HashMap::new();

    for member_path in parse_go_work_use_paths(go_work)
        .into_iter()
        .chain(parse_go_work_replace_paths(go_work))
    {
        let root = resolve_go_work_member_path(repo_root, &member_path);
        if let Some(module_path) = read_go_module_path(&root) {
            packages.insert(module_path, root.clone());
            roots.push(root);
        }
    }

    roots.sort();
    roots.dedup();

    WorkspaceInfo::new(packages, roots)
}

fn discover_single_go_module(repo_root: &Path) -> WorkspaceInfo {
    let Some(module_path) = read_go_module_path(repo_root) else {
        return WorkspaceInfo::default();
    };

    let root = repo_root.to_path_buf();
    let mut packages = HashMap::new();
    packages.insert(module_path, root.clone());

    WorkspaceInfo::new(packages, vec![root])
}

fn parse_go_work_use_paths(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut in_use_block = false;
    let mut results = Vec::new();

    for line in content.lines() {
        let trimmed = strip_go_line_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if in_use_block {
            if trimmed.starts_with(')') {
                in_use_block = false;
                continue;
            }
            if let Some(path) = first_go_token(trimmed.trim_end_matches(')').trim()) {
                results.push(path);
            }
            if trimmed.ends_with(')') {
                in_use_block = false;
            }
            continue;
        }

        let Some(rest) = go_directive_value(trimmed, "use") else {
            continue;
        };
        if rest.starts_with('(') {
            in_use_block = true;
            let inline = rest.trim_start_matches('(').trim();
            if !inline.is_empty()
                && !inline.starts_with(')')
                && let Some(path) = first_go_token(inline.trim_end_matches(')').trim())
            {
                results.push(path);
            }
            if rest.ends_with(')') {
                in_use_block = false;
            }
        } else if let Some(path) = first_go_token(rest) {
            results.push(path);
        }
    }

    results
}

fn parse_go_work_replace_paths(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut in_replace_block = false;
    let mut results = Vec::new();

    for line in content.lines() {
        let trimmed = strip_go_line_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if in_replace_block {
            if trimmed.starts_with(')') {
                in_replace_block = false;
                continue;
            }
            collect_go_replace_path(trimmed.trim_end_matches(')').trim(), &mut results);
            if trimmed.ends_with(')') {
                in_replace_block = false;
            }
            continue;
        }

        let Some(rest) = go_directive_value(trimmed, "replace") else {
            continue;
        };
        if rest.starts_with('(') {
            in_replace_block = true;
            collect_go_replace_path(rest.trim_start_matches('(').trim(), &mut results);
            if rest.ends_with(')') {
                in_replace_block = false;
            }
        } else {
            collect_go_replace_path(rest, &mut results);
        }
    }

    results
}

fn collect_go_replace_path(line: &str, results: &mut Vec<String>) {
    let Some((_, target)) = line.split_once("=>") else {
        return;
    };
    let Some(path) = first_go_token(target) else {
        return;
    };
    if path.starts_with('.') || Path::new(&path).is_absolute() {
        results.push(path);
    }
}

pub(crate) fn read_go_module_path(dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(dir.join("go.mod")).ok()?;
    for line in content.lines() {
        let trimmed = strip_go_line_comment(line).trim();
        let Some(rest) = go_directive_value(trimmed, "module") else {
            continue;
        };
        if let Some(module_path) = first_go_token(rest) {
            return Some(module_path);
        }
    }
    None
}

fn resolve_go_work_member_path(repo_root: &Path, member_path: &str) -> PathBuf {
    let path = Path::new(member_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn go_directive_value<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(directive)?;
    let has_boundary = matches!(rest.chars().next(), Some(c) if c.is_whitespace() || c == '(');
    if !has_boundary {
        return None;
    }
    Some(rest.trim())
}

fn first_go_token(value: &str) -> Option<String> {
    let value = value.trim_start();
    let first = value.chars().next()?;

    let token = match first {
        '"' => parse_double_quoted_go_token(value)?,
        '`' => parse_raw_go_token(value)?,
        _ => value.split_whitespace().next()?.to_string(),
    };

    if token.is_empty() { None } else { Some(token) }
}

fn parse_double_quoted_go_token(value: &str) -> Option<String> {
    let mut escaped = false;
    for (idx, ch) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            let literal = &value[..=idx];
            if let Ok(decoded) = serde_json::from_str(literal) {
                return Some(decoded);
            }
            return Some(value[1..idx].to_string());
        }
    }
    None
}

fn parse_raw_go_token(value: &str) -> Option<String> {
    for (idx, ch) in value.char_indices().skip(1) {
        if ch == '`' {
            return Some(value[1..idx].to_string());
        }
    }
    None
}

fn strip_go_line_comment(line: &str) -> &str {
    let mut in_double_quote = false;
    let mut in_raw_quote = false;
    let mut escaped = false;

    for (idx, ch) in line.char_indices() {
        if in_raw_quote {
            if ch == '`' {
                in_raw_quote = false;
            }
            continue;
        }
        if in_double_quote {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_double_quote = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_double_quote = true,
            '`' => in_raw_quote = true,
            '/' if line[idx..].starts_with("//") => return &line[..idx],
            _ => {}
        }
    }

    line
}
